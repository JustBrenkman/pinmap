use std::fmt;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::{DocKind, PinId, SchemaVersion};

/// Pin definitions: what a chip can do. Normally written by an importer.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct PinDef {
    pub schema: SchemaVersion,
    pub kind: DocKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    pub device: Device,
    /// Package name to (logical pin to physical pad). Pads are strings because
    /// BGA packages use names like `A1`.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub packages: IndexMap<String, IndexMap<PinId, String>>,
    pub pins: IndexMap<PinId, Pin>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct Meta {
    /// Set by importers. Generated files are changed by re-running the importer.
    #[serde(default)]
    pub generated: bool,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct Device {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct Pin {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<Port>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub functions: Vec<Function>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct Port {
    pub bank: String,
    pub index: u8,
}

/// One thing a pin can do. Importers split the vendor's signal string into
/// `periph` and `name`, for example `SPI1` and `SCK`.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct Function {
    pub periph: String,
    pub name: String,
    pub mux: Mux,
}

/// How a function is selected on a pin.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mux {
    /// Alternate-function number (STM32).
    Af(u8),
    /// Function-select value (RP2040).
    FuncSel(u8),
    /// Routable to any pin (nRF).
    Any,
    Analog,
    /// Hard-wired, no mux setting.
    Fixed,
    /// Named register values for vendors that fit none of the above (i.MX
    /// mux and pad settings).
    Vendor(IndexMap<String, u32>),
}

/// A short label: `AF5`, `F3`, `any`, `analog`, `fixed`, or `vendor(k=v, ...)`.
impl fmt::Display for Mux {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Af(n) => write!(f, "AF{n}"),
            Self::FuncSel(n) => write!(f, "F{n}"),
            Self::Any => f.write_str("any"),
            Self::Analog => f.write_str("analog"),
            Self::Fixed => f.write_str("fixed"),
            Self::Vendor(values) => {
                f.write_str("vendor(")?;
                for (i, (key, value)) in values.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{key}={value}")?;
                }
                f.write_str(")")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mux_displays_as_a_short_label() {
        let vendor: IndexMap<String, u32> = [("mux".to_owned(), 1), ("pad".to_owned(), 2)].into();
        let cases = [
            (Mux::Af(5), "AF5"),
            (Mux::FuncSel(3), "F3"),
            (Mux::Any, "any"),
            (Mux::Analog, "analog"),
            (Mux::Fixed, "fixed"),
            (Mux::Vendor(vendor), "vendor(mux=1, pad=2)"),
        ];
        for (mux, expected) in cases {
            assert_eq!(mux.to_string(), expected);
        }
    }
}
