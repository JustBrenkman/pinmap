use std::fmt;

use serde::{Deserialize, Serialize};

/// The only schema version this crate reads and writes.
pub const SCHEMA_V1: &str = "pinmap/v1";

/// The `schema` field of a document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SchemaVersion(String);

impl SchemaVersion {
    pub fn v1() -> Self {
        Self(SCHEMA_V1.to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_supported(&self) -> bool {
        self.0 == SCHEMA_V1
    }
}

impl Default for SchemaVersion {
    fn default() -> Self {
        Self::v1()
    }
}

/// A logical pin name such as `PA5`, as the vendor spells it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PinId(String);

impl PinId {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for PinId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl From<&str> for PinId {
    fn from(name: &str) -> Self {
        Self::new(name)
    }
}

impl From<String> for PinId {
    fn from(name: String) -> Self {
        Self::new(name)
    }
}

impl fmt::Display for PinId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Pull resistor configuration. "No bias" is `floating`, not `none`, because
/// RON can read `None` as an absent option.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Bias {
    Floating,
    PullUp,
    PullDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Drive {
    PushPull,
    OpenDrain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Speed {
    Low,
    Medium,
    High,
    VeryHigh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Low,
    High,
}

/// The role of a pin that is not carrying a peripheral signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    GpioIn,
    GpioOut,
    Analog,
    /// No GPIO role (SWD and similar), listed so the pin does not show as free.
    Reserved,
}
