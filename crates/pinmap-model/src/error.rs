use std::path::PathBuf;

use crate::{DocKind, Format};

/// Errors from classifying, reading, writing, and resolving documents.
///
/// Every variant has a stable code, available from [`ModelError::code`].
#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("{}: file name must be <name>.<pindef|pinasg>.<json|ron>", .path.display())]
    Unclassifiable { path: PathBuf },

    #[error("expected a {expected} document, found {found}")]
    KindMismatch { expected: DocKind, found: DocKind },

    #[error("unsupported schema {found:?}, expected {:?}", crate::SCHEMA_V1)]
    UnsupportedSchema { found: String },

    #[error(
        "reference {} is ambiguous: both {} and {} exist",
        .reference.display(), .candidates[0].display(), .candidates[1].display()
    )]
    AmbiguousRef {
        reference: PathBuf,
        candidates: [PathBuf; 2],
    },

    #[error("reference {} not found, looked for {}", .reference.display(), display_paths(.tried))]
    RefNotFound {
        reference: PathBuf,
        tried: Vec<PathBuf>,
    },

    #[error("invalid {format}: {message}")]
    Parse { format: Format, message: String },

    #[error("support for {format} is not compiled in")]
    FormatDisabled { format: Format },

    #[error("{0}")]
    Io(#[from] std::io::Error),

    #[error("{}: {source}", .path.display())]
    At {
        path: PathBuf,
        source: Box<ModelError>,
    },
}

impl ModelError {
    /// The stable diagnostic code for this error.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unclassifiable { .. } => "PM0001",
            Self::KindMismatch { .. } => "PM0002",
            Self::UnsupportedSchema { .. } => "PM0003",
            Self::AmbiguousRef { .. } => "PM0004",
            Self::RefNotFound { .. } => "PM0005",
            Self::Parse { .. } => "PM0006",
            Self::FormatDisabled { .. } => "PM0007",
            Self::Io(_) => "PM0008",
            Self::At { source, .. } => source.code(),
        }
    }

    pub(crate) fn at(self, path: impl Into<PathBuf>) -> Self {
        Self::At {
            path: path.into(),
            source: Box::new(self),
        }
    }
}

fn display_paths(paths: &[PathBuf]) -> String {
    let shown: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
    shown.join(", ")
}
