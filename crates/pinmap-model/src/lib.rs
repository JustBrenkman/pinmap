//! Document types and serialization for pinmap.
//!
//! Two document kinds exist, each stored as JSON or RON:
//!
//! - [`PinDef`]: what a chip can do (pins, functions, mux values, package pads).
//! - [`PinAsg`]: what a board chose (signal-to-pin mapping, net labels, config).
//!
//! A file is named `<name>.<kind>.<format>`, for example `revb.pinasg.ron`.
//! [`classify`] reads the kind and format from the name.
//!
//! ```no_run
//! use pinmap_model::{PinAsg, read, write};
//!
//! let asg: PinAsg = read("board/revb.pinasg.ron")?;
//! write("board/revb.pinasg.json", &asg)?;
//! # Ok::<(), pinmap_model::ModelError>(())
//! ```
//!
//! This crate holds the types and their file representation only. Loading a
//! project, following imports, and validating assignments live in `pinmap-core`.

mod comments;
mod common;
mod error;
mod io;
mod kind;
mod paths;
mod pinasg;
mod pindef;
mod reference;
#[cfg(feature = "schema")]
mod schema;

pub use comments::has_comments;
pub use common::{Bias, Drive, Level, Mode, PinId, SCHEMA_V1, SchemaVersion, Speed};
pub use error::ModelError;
pub use io::{AnyDoc, Document, from_str, read, read_any, to_string, write, write_atomic};
pub use kind::{DocKind, Format, classify};
pub use paths::normalize_path;
pub use pinasg::{PeriphAsg, PinAsg, PinAsgEntry, SignalAsg, Target};
pub use pindef::{Device, Function, Meta, Mux, Pin, PinDef, Port};
pub use reference::resolve_ref;
#[cfg(feature = "schema")]
pub use schema::json_schema;
