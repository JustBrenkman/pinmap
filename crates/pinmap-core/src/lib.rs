//! Loading, resolving, and checking pinmap projects.
//!
//! [`open`] is the entry point: it loads a root pinasg with its imports and
//! pindef, merges them into a [`Resolved`] view, and runs every check.

mod diagnostic;
mod discover;
mod load;
mod resolve;
mod rules;

pub use diagnostic::{Counts, Diagnostic, Severity, sort};
pub use discover::{DiscoverError, discover_root};
pub use load::{LoadedPinAsg, LoadedPinDef, Project, load};
pub use resolve::{
    Origin, Resolved, ResolvedPeriph, ResolvedPin, ResolvedSignal, Usage, open, resolve,
};
