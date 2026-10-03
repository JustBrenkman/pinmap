use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use pinmap_core::Diagnostic;
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
