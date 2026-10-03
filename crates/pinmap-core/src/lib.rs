//! Loading, resolving, and checking pinmap projects.
//!
//! [`open`] is the entry point: it loads a root pinasg with its imports and
//! pindef, merges them into a [`Resolved`] view, and runs every check.

mod diagnostic;
mod discover;
mod edit;
mod emit;
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
/// `items` joined with commas, or `none` when there are none.
pub(crate) fn list_or_none(items: impl IntoIterator<Item = impl std::fmt::Display>) -> String {
    let items: Vec<String> = items.into_iter().map(|item| item.to_string()).collect();
    if items.is_empty() {
        "none".to_owned()
    } else {
        items.join(", ")
    }
}

pub use emit::{
    EmitError, EmitOptions, EmitOutput, Emitter, OptionSpec, OutputFile, bool_option, check_options,
};
pub use load::{LoadedPinAsg, LoadedPinDef, Project, load};
pub use resolve::{
    Assignment, Origin, Resolved, ResolvedPeriph, ResolvedPin, ResolvedSignal, Usage, open, resolve,
};
pub use suggest::suggest;
