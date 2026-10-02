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
//! This crate holds the types and their file representation only. Loading a
//! project, following imports, and validating assignments live in `pinmap-core`.

mod common;
mod error;
mod kind;
mod pinasg;
mod pindef;

pub use common::{Bias, Drive, Level, Mode, PinId, SCHEMA_V1, SchemaVersion, Speed};
pub use error::ModelError;
pub use kind::{DocKind, Format, classify};
pub use pinasg::{PeriphAsg, PinAsg, PinAsgEntry, SignalAsg, Target};
pub use pindef::{Device, Function, Meta, Mux, Pin, PinDef, Port};
