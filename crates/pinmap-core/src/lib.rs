//! Loading, resolving, and checking pinmap projects.
//!
//! [`open`] is the entry point: it loads a root pinasg with its imports and
//! pindef, merges them into a [`Resolved`] view, and runs every check.

mod diagnostic;
mod discover;
mod edit;
mod load;
mod resolve;
mod rules;
mod suggest;

pub use diagnostic::{Counts, Diagnostic, Severity, sort};
pub use discover::{DiscoverError, discover_root};
pub use edit::{
    EditError, EditOptions, EditOutcome, FileChange, PinChange, Removed, Session, SignalConfig,
    SignalSpec,
};
pub use load::{LoadedPinAsg, LoadedPinDef, Project, load};
pub use resolve::{
    Origin, Resolved, ResolvedPeriph, ResolvedPin, ResolvedSignal, Usage, open, resolve,
};
pub use suggest::suggest;
