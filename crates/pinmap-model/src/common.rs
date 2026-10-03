use std::fmt;

use serde::{Deserialize, Serialize};

/// The only schema version this crate reads and writes.
pub const SCHEMA_V1: &str = "pinmap/v1";

/// The `schema` field of a document.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Bias {
    Floating,
    PullUp,
    PullDown,
}

impl Bias {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Floating => "floating",
            Self::PullUp => "pull_up",
            Self::PullDown => "pull_down",
        }
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Drive {
    PushPull,
    OpenDrain,
}

impl Drive {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PushPull => "push_pull",
            Self::OpenDrain => "open_drain",
        }
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Speed {
    Low,
    Medium,
    High,
    VeryHigh,
}

impl Speed {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::VeryHigh => "very_high",
        }
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Low,
    High,
}

impl Level {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::High => "high",
        }
    }
}

/// The role of a pin that is not carrying a peripheral signal.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    GpioIn,
    GpioOut,
    Analog,
    /// No GPIO role (SWD and similar), listed so the pin does not show as free.
    Reserved,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GpioIn => "gpio_in",
            Self::GpioOut => "gpio_out",
            Self::Analog => "analog",
            Self::Reserved => "reserved",
        }
    }
}

#[cfg(all(test, feature = "json"))]
mod tests {
    use super::*;

    fn serde_name<T: Serialize>(value: T) -> String {
        serde_json::to_value(value)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned()
    }

    #[test]
    fn as_str_matches_serde_names() {
        for v in [Bias::Floating, Bias::PullUp, Bias::PullDown] {
            assert_eq!(v.as_str(), serde_name(v));
        }
        for v in [Drive::PushPull, Drive::OpenDrain] {
            assert_eq!(v.as_str(), serde_name(v));
        }
        for v in [Speed::Low, Speed::Medium, Speed::High, Speed::VeryHigh] {
            assert_eq!(v.as_str(), serde_name(v));
        }
        for v in [Level::Low, Level::High] {
            assert_eq!(v.as_str(), serde_name(v));
        }
        for v in [Mode::GpioIn, Mode::GpioOut, Mode::Analog, Mode::Reserved] {
            assert_eq!(v.as_str(), serde_name(v));
        }
    }
}
