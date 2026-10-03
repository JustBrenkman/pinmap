use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::ModelError;

/// Which document a file holds.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocKind {
    Pindef,
    Pinasg,
}

impl DocKind {
    /// The name used in file names and in the `kind` field.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pindef => "pindef",
            Self::Pinasg => "pinasg",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "pindef" => Some(Self::Pindef),
            "pinasg" => Some(Self::Pinasg),
            _ => None,
        }
    }
}

impl fmt::Display for DocKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The text format a file is stored in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    Json,
    Ron,
}

impl Format {
    /// The file extension, without a leading dot.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Ron => "ron",
        }
    }

    fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "json" => Some(Self::Json),
            "ron" => Some(Self::Ron),
            _ => None,
        }
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.extension())
    }
}

/// Reads the document kind and format from a file name of the form
/// `<name>.<kind>.<format>`. The name may itself contain dots.
pub fn classify(path: &Path) -> Result<(DocKind, Format), ModelError> {
    let parsed = || {
        let file_name = path.file_name()?.to_str()?;
        let (rest, ext) = file_name.rsplit_once('.')?;
        let (name, kind) = rest.rsplit_once('.')?;
        if name.is_empty() {
            return None;
        }
        Some((DocKind::from_name(kind)?, Format::from_extension(ext)?))
    };
    parsed().ok_or_else(|| ModelError::Unclassifiable {
        path: path.to_path_buf(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_valid_names() {
        let cases = [
            ("x.pindef.json", DocKind::Pindef, Format::Json),
            ("x.pindef.ron", DocKind::Pindef, Format::Ron),
            ("x.pinasg.json", DocKind::Pinasg, Format::Json),
            ("a.b.pinasg.ron", DocKind::Pinasg, Format::Ron),
            ("board/rev.b/revb.pinasg.ron", DocKind::Pinasg, Format::Ron),
        ];
        for (name, kind, format) in cases {
            assert_eq!(classify(Path::new(name)).unwrap(), (kind, format), "{name}");
        }
    }

    #[test]
    fn rejects_invalid_names() {
        let cases = [
            "x.ron",
            "x.json",
            "x.pinasg.yaml",
            "x.pinasg",
            "x.board.ron",
            ".pinasg.ron",
            "pinasg.ron",
            "x.PINASG.ron",
            "",
        ];
        for name in cases {
            let err = classify(Path::new(name)).unwrap_err();
            assert_eq!(err.code(), "PM0001", "{name}");
        }
    }
}
