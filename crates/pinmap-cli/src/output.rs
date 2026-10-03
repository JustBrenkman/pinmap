use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use pinmap_core::Diagnostic;
use pinmap_model::Mux;
use serde::Serialize;

pub struct Ctx {
    pub json: bool,
    pub project: Option<PathBuf>,
}

/// How a command finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// Ran and found nothing wrong.
    Ok,
    /// Ran and found problems.
    Problems,
    /// Could not run.
    Failed,
}

impl From<Exit> for ExitCode {
    fn from(exit: Exit) -> Self {
        ExitCode::from(match exit {
            Exit::Ok => 0,
            Exit::Problems => 1,
            Exit::Failed => 2,
        })
    }
}

/// A command result that can also print itself as text.
pub trait Render: Serialize {
    fn render(&self, out: &mut String);
}

impl Ctx {
    pub fn emit<R: Render>(&self, report: &R) -> anyhow::Result<()> {
        let mut text = String::new();
        if self.json {
            text = serde_json::to_string_pretty(report)?;
            text.push('\n');
        } else {
            report.render(&mut text);
        }
        print!("{text}");
        Ok(())
    }
}

/// Writes rows as left-aligned columns separated by two spaces.
pub fn table(out: &mut String, header: &[&str], rows: &[Vec<String>]) {
    let mut widths: Vec<usize> = header.iter().map(|h| h.chars().count()).collect();
    for row in rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    let mut line = |cells: &[&str]| {
        let mut text = String::new();
        for (i, cell) in cells.iter().enumerate() {
            if i > 0 {
                text.push_str("  ");
            }
            text.push_str(cell);
            text.extend(std::iter::repeat_n(' ', widths[i] - cell.chars().count()));
        }
        out.push_str(text.trim_end());
        out.push('\n');
    };
    line(header);
    for row in rows {
        let cells: Vec<&str> = row.iter().map(String::as_str).collect();
        line(&cells);
    }
}

pub fn dash(value: Option<&str>) -> String {
    value.unwrap_or("-").to_owned()
}

/// `AF5`, `F3`, `analog`, `vendor(mux=1, pad=2)`.
pub fn mux_text(mux: &Mux) -> String {
    match mux {
        Mux::Af(n) => format!("AF{n}"),
        Mux::FuncSel(n) => format!("F{n}"),
        Mux::Any => "any".into(),
        Mux::Analog => "analog".into(),
        Mux::Fixed => "fixed".into(),
        Mux::Vendor(values) => {
            let values: Vec<String> = values.iter().map(|(k, v)| format!("{k}={v}")).collect();
            format!("vendor({})", values.join(", "))
        }
    }
}

/// `error[PM0102] message`, then the file and logical path, then the hint,
/// indented under the message.
pub fn render_diagnostic(d: &Diagnostic, out: &mut String) {
    let head = format!("{}[{}] ", d.severity, d.code);
    let indent = " ".repeat(head.chars().count());
    let _ = writeln!(out, "{head}{}", d.message);
    let file = d.file.as_ref().map(|f| f.display().to_string());
    let place: Vec<String> = [file, d.path.clone()].into_iter().flatten().collect();
    if !place.is_empty() {
        let _ = writeln!(out, "{indent}{}", place.join("  "));
    }
    if let Some(hint) = &d.hint {
        let _ = writeln!(out, "{indent}hint: {hint}");
    }
}

/// `key=value` for each setting that is present.
pub fn config<const N: usize>(items: [(&str, Option<&str>); N]) -> Vec<String> {
    items
        .into_iter()
        .filter_map(|(key, value)| value.map(|v| format!("{key}={v}")))
        .collect()
}

/// The name closest to `wanted`, ignoring case. Short names such as `PA5`
/// allow one edit, longer ones two, so unrelated names are not suggested.
pub fn suggest<'a>(wanted: &str, names: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let wanted = wanted.to_ascii_uppercase();
    let limit = if wanted.chars().count() <= 4 { 1 } else { 2 };
    names
        .into_iter()
        .map(|name| (edit_distance(&wanted, &name.to_ascii_uppercase()), name))
        .filter(|(distance, _)| *distance <= limit)
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, name)| name)
}

/// Levenshtein distance between two strings, by characters.
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut current = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let substitute = previous[j] + usize::from(ca != *cb);
            current.push(substitute.min(previous[j + 1] + 1).min(current[j] + 1));
        }
        previous = current;
    }
    previous[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggests_the_nearest_name() {
        let names = ["PA2", "PA5", "PB12"];
        assert_eq!(suggest("pa5", names), Some("PA5"));
        assert_eq!(suggest("PA55", names), Some("PA5"));
        assert_eq!(suggest("PC9", names), None);
        assert_eq!(suggest("OTG_FX", ["OTG_FS", "SPI1"]), Some("OTG_FS"));
        assert_eq!(edit_distance("kitten", "sitting"), 3);
    }
}
