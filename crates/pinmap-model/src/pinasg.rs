use std::path::PathBuf;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::{Bias, DocKind, Drive, Level, Mode, PinId, SchemaVersion, Speed};

/// Pin assignments: what a board chose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct PinAsg {
    pub schema: SchemaVersion,
    pub kind: DocKind,
    /// Reference to the pindef, as a path or a stem.
    pub device: PathBuf,
    pub target: Target,
    /// References to other pinasg files, merged by union.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub imports: Vec<PathBuf>,
    /// Peripheral name to its assignment. Presence means enabled.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub peripherals: IndexMap<String, PeriphAsg>,
    /// Pins used directly rather than through a peripheral signal.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub pins: IndexMap<PinId, PinAsgEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct Target {
    pub package: String,
    /// The chip's reference designator in the schematic, for example `U1`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refdes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct PeriphAsg {
    pub signals: IndexMap<String, SignalAsg>,
    /// Devicetree properties as raw devicetree text, for example `"<8000000>"`.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub props: IndexMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct SignalAsg {
    pub pin: PinId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub net: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bias: Option<Bias>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drive: Option<Drive>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<Speed>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct PinAsgEntry {
    pub mode: Mode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial: Option<Level>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bias: Option<Bias>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub net: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}
