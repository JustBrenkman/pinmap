//! Emitters that turn a resolved project into output files.
//!
//! Each emitter is a module behind a cargo feature. [`all`] lists the ones
//! compiled in; the CLI picks from it with `pinmap emit --to <id>`.

#[cfg(feature = "json")]
pub mod json;
#[cfg(feature = "markdown")]
pub mod markdown;
mod table;

use pinmap_core::Emitter;

/// Every emitter compiled into this build.
#[allow(
    clippy::vec_init_then_push,
    reason = "each push is behind its own feature"
)]
pub fn all() -> Vec<Box<dyn Emitter>> {
    let mut emitters: Vec<Box<dyn Emitter>> = Vec::new();
    #[cfg(feature = "markdown")]
    emitters.push(Box::new(markdown::Markdown));
    #[cfg(feature = "json")]
    emitters.push(Box::new(json::Json));
    emitters
}
