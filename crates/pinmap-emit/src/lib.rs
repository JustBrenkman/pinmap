//! Emitters that turn a resolved project into output files.
//!
//! Each emitter is a module behind a cargo feature. [`all`] lists the ones
//! compiled in; the CLI picks from it with `pinmap emit --to <id>`.

#[cfg(feature = "markdown")]
pub mod markdown;
mod table;

use pinmap_core::Emitter;

/// Every emitter compiled into this build.
pub fn all() -> Vec<Box<dyn Emitter>> {
    let mut emitters: Vec<Box<dyn Emitter>> = Vec::new();
    #[cfg(feature = "markdown")]
    emitters.push(Box::new(markdown::Markdown));
    emitters
}
