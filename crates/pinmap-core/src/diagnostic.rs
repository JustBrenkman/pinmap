use std::cmp::Ordering;
use std::fmt;
use std::path::PathBuf;

use pinmap_model::ModelError;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        })
    }
}

/// One finding, with a stable code and where it was found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    pub code: &'static str,
    pub severity: Severity,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
    /// Logical path inside the file, for example `peripherals.SPI1.signals.MOSI`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// A command to run next.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl Diagnostic {
    fn new(code: &'static str, severity: Severity, message: impl Into<String>) -> Self {
        Self {
            code,
            severity,
            message: message.into(),
            file: None,
            path: None,
            hint: None,
        }
    }

    pub fn error(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(code, Severity::Error, message)
    }

    pub fn warning(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(code, Severity::Warning, message)
    }

    pub fn info(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(code, Severity::Info, message)
    }

    pub fn in_file(mut self, file: impl Into<PathBuf>) -> Self {
        self.file = Some(file.into());
        self
    }

    pub fn at_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// An error from reading a file. The file moves out of the message into
    /// [`Diagnostic::file`].
    pub fn from_model(err: &ModelError) -> Self {
        match err {
            ModelError::At { path, error } => {
                Self::error(err.code(), error.to_string()).in_file(path)
            }
            other => Self::error(other.code(), other.to_string()),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Counts {
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
}

impl Counts {
    pub fn of(diagnostics: &[Diagnostic]) -> Self {
        let mut counts = Self::default();
        for diagnostic in diagnostics {
            match diagnostic.severity {
                Severity::Error => counts.errors += 1,
                Severity::Warning => counts.warnings += 1,
                Severity::Info => counts.infos += 1,
            }
        }
        counts
    }
}

/// Puts diagnostics in a stable order: by file, then logical path, then code.
/// Numbers inside names compare by value, so `imports[2]` precedes `imports[10]`.
pub fn sort(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by(|a, b| {
        let file = |d: &Diagnostic| d.file.as_ref().map(|f| f.to_string_lossy().into_owned());
        cmp_opt(file(a).as_deref(), file(b).as_deref())
            .then_with(|| cmp_opt(a.path.as_deref(), b.path.as_deref()))
            .then_with(|| a.code.cmp(b.code))
    });
}

/// Missing values sort first.
fn cmp_opt(a: Option<&str>, b: Option<&str>) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => natural_cmp(a, b),
        _ => a.is_some().cmp(&b.is_some()),
    }
}

/// Compares strings with runs of ASCII digits compared as numbers.
fn natural_cmp(mut a: &str, mut b: &str) -> Ordering {
    loop {
        let (Some(x), Some(y)) = (a.chars().next(), b.chars().next()) else {
            return a.len().cmp(&b.len());
        };
        if x.is_ascii_digit() && y.is_ascii_digit() {
            let (digits_a, rest_a) = split_digits(a);
            let (digits_b, rest_b) = split_digits(b);
            let (na, nb) = (
                digits_a.trim_start_matches('0'),
                digits_b.trim_start_matches('0'),
            );
            let order = na.len().cmp(&nb.len()).then_with(|| na.cmp(nb));
            if order != Ordering::Equal {
                return order;
            }
            (a, b) = (rest_a, rest_b);
        } else {
            if x != y {
                return x.cmp(&y);
            }
            (a, b) = (&a[x.len_utf8()..], &b[y.len_utf8()..]);
        }
    }
}

fn split_digits(s: &str) -> (&str, &str) {
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    s.split_at(end)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use pinmap_model::{PinDef, read};

    use super::*;

    #[test]
    fn model_errors_keep_code_and_file() {
        let err = read::<PinDef>(Path::new("missing/chip.pindef.json")).unwrap_err();
        let diag = Diagnostic::from_model(&err);
        assert_eq!(diag.code, "PM0008");
        assert_eq!(diag.severity, Severity::Error);
        assert_eq!(
            diag.file.as_deref(),
            Some(Path::new("missing/chip.pindef.json"))
        );
        assert!(!diag.message.contains("missing/chip"), "{}", diag.message);
    }

    #[test]
    fn counts_by_severity() {
        let diags = [
            Diagnostic::error("PM0101", "a"),
            Diagnostic::error("PM0102", "b"),
            Diagnostic::info("PM0110", "c"),
        ];
        let counts = Counts::of(&diags);
        assert_eq!((counts.errors, counts.warnings, counts.infos), (2, 0, 1));
    }

    #[test]
    fn numbers_in_paths_sort_naturally() {
        let mut diags = vec![
            Diagnostic::error("PM0005", "x")
                .in_file("b.pinasg.json")
                .at_path("imports[10]"),
            Diagnostic::error("PM0005", "x")
                .in_file("b.pinasg.json")
                .at_path("imports[2]"),
            Diagnostic::error("PM0101", "x")
                .in_file("b.pinasg.json")
                .at_path("pins.PA10"),
            Diagnostic::error("PM0101", "x")
                .in_file("b.pinasg.json")
                .at_path("pins.PA9"),
        ];
        sort(&mut diags);
        let paths: Vec<&str> = diags.iter().map(|d| d.path.as_deref().unwrap()).collect();
        assert_eq!(
            paths,
            ["imports[2]", "imports[10]", "pins.PA9", "pins.PA10"]
        );
        assert_eq!(natural_cmp("a01", "a1"), std::cmp::Ordering::Equal);
    }

    #[test]
    fn sorts_by_file_then_path_then_code() {
        let mut diags = vec![
            Diagnostic::error("PM0102", "x")
                .in_file("b.pinasg.json")
                .at_path("pins.PA1"),
            Diagnostic::error("PM0104", "x")
                .in_file("a.pinasg.json")
                .at_path("pins.PA2"),
            Diagnostic::error("PM0101", "x")
                .in_file("a.pinasg.json")
                .at_path("pins.PA2"),
            Diagnostic::error("PM0107", "x"),
        ];
        sort(&mut diags);
        let order: Vec<&str> = diags.iter().map(|d| d.code).collect();
        assert_eq!(order, ["PM0107", "PM0101", "PM0104", "PM0102"]);
    }
}
