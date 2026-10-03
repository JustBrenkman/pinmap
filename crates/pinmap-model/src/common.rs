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

/// An enum of settings stored by name in files. `as_str` returns the same
/// name serde reads and writes.
macro_rules! setting {
    ($(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident = $text:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum $name {
            $($(#[$vmeta])* #[serde(rename = $text)] $variant),+
        }

        impl $name {
            pub fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $text),+
                }
            }
        }
    };
}

setting! {
    /// Pull resistor configuration. "No bias" is `floating`, not `none`, because
    /// RON can read `None` as an absent option.
    Bias { Floating = "floating", PullUp = "pull_up", PullDown = "pull_down" }
}

setting! {
    Drive { PushPull = "push_pull", OpenDrain = "open_drain" }
}

setting! {
    Speed { Low = "low", Medium = "medium", High = "high", VeryHigh = "very_high" }
}

setting! {
    Level { Low = "low", High = "high" }
}

setting! {
    /// The role of a pin that is not carrying a peripheral signal.
    Mode {
        GpioIn = "gpio_in",
        GpioOut = "gpio_out",
        Analog = "analog",
        /// No GPIO role (SWD and similar), listed so the pin does not show as free.
        Reserved = "reserved",
    }
}
