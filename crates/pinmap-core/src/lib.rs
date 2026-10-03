//! Loading, resolving, and checking pinmap projects.
//!
//! [`open`] is the entry point: it loads a root pinasg with its imports and
//! pindef, merges them into a [`Resolved`] view, and runs every check.

mod diagnostic;

pub use diagnostic::{Counts, Diagnostic, Severity, sort};
