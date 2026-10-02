use std::fs;
use std::path::Path;

use serde::Deserialize;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::{DocKind, Format, ModelError, PinAsg, PinDef, SchemaVersion, classify};

/// A document type that can be read from and written to a file.
pub trait Document: Serialize + DeserializeOwned {
    /// The kind this type represents.
    const KIND: DocKind;

    /// The value of the document's own `kind` field.
    fn kind(&self) -> DocKind;

    /// The value of the document's own `schema` field.
    fn schema(&self) -> &SchemaVersion;
}

impl Document for PinDef {
    const KIND: DocKind = DocKind::Pindef;

    fn kind(&self) -> DocKind {
        self.kind
    }

    fn schema(&self) -> &SchemaVersion {
        &self.schema
    }
}

impl Document for PinAsg {
    const KIND: DocKind = DocKind::Pinasg;

    fn kind(&self) -> DocKind {
        self.kind
    }

    fn schema(&self) -> &SchemaVersion {
        &self.schema
    }
}

/// The two fields every document starts with. Parsed first so that a wrong
/// kind or schema is reported as such, not as a pile of unknown-field errors.
#[derive(Deserialize)]
struct Header {
    schema: SchemaVersion,
    kind: DocKind,
}

/// Parses a document from text in the given format.
pub fn from_str<T: Document>(text: &str, format: Format) -> Result<T, ModelError> {
    let header: Header = parse(text, format)?;
    check_header::<T>(&header.schema, header.kind)?;
    parse(text, format)
}

/// Serializes a document to canonical text: fixed indentation, fields in
/// declaration order, map entries in their stored order, and a trailing
/// newline. Converting between formats and back gives identical bytes.
pub fn to_string<T: Document>(doc: &T, format: Format) -> Result<String, ModelError> {
    check_header::<T>(doc.schema(), doc.kind())?;
    let mut text = print(doc, format)?;
    if !text.ends_with('\n') {
        text.push('\n');
    }
    Ok(text)
}

/// Reads a document from a file. The file name decides the format and must
/// name the kind that `T` represents.
pub fn read<T: Document>(path: impl AsRef<Path>) -> Result<T, ModelError> {
    let path = path.as_ref();
    let load = || {
        let format = classify_as::<T>(path)?;
        let text = fs::read_to_string(path)?;
        from_str(&text, format)
    };
    load().map_err(|err| at(err, path))
}

/// Writes a document to a file in canonical form. The file name decides the
/// format and must name the kind that `T` represents.
pub fn write<T: Document>(path: impl AsRef<Path>, doc: &T) -> Result<(), ModelError> {
    let path = path.as_ref();
    let save = || {
        let format = classify_as::<T>(path)?;
        let text = to_string(doc, format)?;
        fs::write(path, text)?;
        Ok(())
    };
    save().map_err(|err| at(err, path))
}

fn at(err: ModelError, path: &Path) -> ModelError {
    match err {
        // Already names the path.
        ModelError::Unclassifiable { .. } => err,
        other => other.at(path),
    }
}

fn classify_as<T: Document>(path: &Path) -> Result<Format, ModelError> {
    let (kind, format) = classify(path)?;
    if kind != T::KIND {
        return Err(ModelError::KindMismatch {
            expected: T::KIND,
            found: kind,
        });
    }
    Ok(format)
}

fn check_header<T: Document>(schema: &SchemaVersion, kind: DocKind) -> Result<(), ModelError> {
    if !schema.is_supported() {
        return Err(ModelError::UnsupportedSchema {
            found: schema.as_str().to_owned(),
        });
    }
    if kind != T::KIND {
        return Err(ModelError::KindMismatch {
            expected: T::KIND,
            found: kind,
        });
    }
    Ok(())
}

fn parse<T: DeserializeOwned>(text: &str, format: Format) -> Result<T, ModelError> {
    match format {
        Format::Json => json::parse(text),
        Format::Ron => ron::parse(text),
    }
}

fn print<T: Serialize>(doc: &T, format: Format) -> Result<String, ModelError> {
    match format {
        Format::Json => json::print(doc),
        Format::Ron => ron::print(doc),
    }
}

#[cfg(feature = "json")]
mod json {
    use super::*;

    pub fn parse<T: DeserializeOwned>(text: &str) -> Result<T, ModelError> {
        serde_json::from_str(text).map_err(error)
    }

    pub fn print<T: Serialize>(doc: &T) -> Result<String, ModelError> {
        serde_json::to_string_pretty(doc).map_err(error)
    }

    fn error(err: serde_json::Error) -> ModelError {
        ModelError::Parse {
            format: Format::Json,
            message: err.to_string(),
        }
    }
}

#[cfg(feature = "ron")]
mod ron {
    use ::ron::extensions::Extensions;
    use ::ron::ser::PrettyConfig;

    use super::*;

    pub fn parse<T: DeserializeOwned>(text: &str) -> Result<T, ModelError> {
        // Accept files with or without the `#![enable(implicit_some)]` line.
        ::ron::Options::default()
            .with_default_extension(Extensions::IMPLICIT_SOME)
            .from_str(text)
            .map_err(error)
    }

    pub fn print<T: Serialize>(doc: &T) -> Result<String, ModelError> {
        let config = PrettyConfig::new()
            .new_line("\n")
            .indentor("    ")
            .struct_names(false)
            .extensions(Extensions::IMPLICIT_SOME);
        ::ron::ser::to_string_pretty(doc, config).map_err(error)
    }

    fn error(err: impl std::fmt::Display) -> ModelError {
        ModelError::Parse {
            format: Format::Ron,
            message: err.to_string(),
        }
    }
}

#[cfg(not(feature = "json"))]
mod json {
    use super::*;

    pub fn parse<T>(_text: &str) -> Result<T, ModelError> {
        Err(ModelError::FormatDisabled {
            format: Format::Json,
        })
    }

    pub fn print<T>(_doc: &T) -> Result<String, ModelError> {
        Err(ModelError::FormatDisabled {
            format: Format::Json,
        })
    }
}

#[cfg(not(feature = "ron"))]
mod ron {
    use super::*;

    pub fn parse<T>(_text: &str) -> Result<T, ModelError> {
        Err(ModelError::FormatDisabled {
            format: Format::Ron,
        })
    }

    pub fn print<T>(_doc: &T) -> Result<String, ModelError> {
        Err(ModelError::FormatDisabled {
            format: Format::Ron,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Bias, Mux, PinAsg, PinDef};

    #[cfg(feature = "json")]
    const PINDEF_JSON: &str = r#"{
  "schema": "pinmap/v1",
  "kind": "pindef",
  "device": { "name": "STM32F407VGTx", "vendor": "st" },
  "pins": {
    "PA5": {
      "port": { "bank": "A", "index": 5 },
      "functions": [
        { "periph": "SPI1", "name": "SCK", "mux": { "af": 5 } },
        { "periph": "DAC", "name": "OUT2", "mux": "analog" },
        { "periph": "X", "name": "Y", "mux": { "vendor": { "mux_reg": 20, "pad": 3 } } }
      ]
    }
  }
}"#;

    #[cfg(feature = "ron")]
    const PINDEF_RON: &str = r#"(
    schema: "pinmap/v1",
    kind: pindef,
    device: (name: "STM32F407VGTx", vendor: "st"),
    pins: {
        "PA5": (
            port: (bank: "A", index: 5),
            functions: [
                (periph: "SPI1", name: "SCK", mux: af(5)),
                (periph: "DAC", name: "OUT2", mux: analog),
                (periph: "X", name: "Y", mux: vendor({"mux_reg": 20, "pad": 3})),
            ],
        ),
    },
)"#;

    #[cfg(feature = "ron")]
    const PINASG_RON: &str = r#"#![enable(implicit_some)]
(
    schema: "pinmap/v1",
    kind: pinasg,
    device: "stm32f407vg",
    target: (package: "LQFP100"),
    peripherals: {
        "USART2": (
            signals: {
                "TX": (pin: "PA2", bias: floating),
                "RX": (pin: "PA3", net: "UART_RX", bias: pull_up),
            },
        ),
    },
)"#;

    fn muxes(def: &PinDef) -> Vec<&Mux> {
        let pin = &def.pins["PA5"];
        pin.functions.iter().map(|f| &f.mux).collect()
    }

    fn assert_muxes(def: &PinDef) {
        let muxes = muxes(def);
        assert_eq!(muxes[0], &Mux::Af(5));
        assert_eq!(muxes[1], &Mux::Analog);
        let Mux::Vendor(values) = muxes[2] else {
            panic!("expected vendor mux, got {:?}", muxes[2]);
        };
        assert_eq!(values["mux_reg"], 20);
    }

    #[cfg(feature = "json")]
    #[test]
    fn json_mux_is_externally_tagged() {
        let def: PinDef = from_str(PINDEF_JSON, Format::Json).unwrap();
        assert_muxes(&def);
        let text = to_string(&def, Format::Json).unwrap();
        assert!(text.contains(r#""af": 5"#), "{text}");
        assert!(text.contains(r#""mux": "analog""#), "{text}");
        assert!(text.ends_with("}\n"));
    }

    #[cfg(feature = "ron")]
    #[test]
    fn ron_mux_is_externally_tagged() {
        let def: PinDef = from_str(PINDEF_RON, Format::Ron).unwrap();
        assert_muxes(&def);
        let text = to_string(&def, Format::Ron).unwrap();
        assert!(text.contains("mux: af(5)"), "{text}");
        assert!(text.contains("mux: analog"), "{text}");
    }

    #[cfg(feature = "ron")]
    #[test]
    fn ron_options_need_no_some() {
        let asg: PinAsg = from_str(PINASG_RON, Format::Ron).unwrap();
        let signals = &asg.peripherals["USART2"].signals;
        assert_eq!(signals["TX"].bias, Some(Bias::Floating));
        assert_eq!(signals["TX"].net, None);
        assert_eq!(signals["RX"].net.as_deref(), Some("UART_RX"));
        assert_eq!(asg.target.refdes, None);

        let text = to_string(&asg, Format::Ron).unwrap();
        assert!(text.starts_with("#![enable(implicit_some)]\n"), "{text}");
        assert!(text.contains("bias: floating"), "{text}");
        assert!(!text.contains("Some"), "{text}");
        assert!(!text.contains("None"), "{text}");
        assert!(!text.contains("refdes"), "{text}");
        assert!(text.ends_with(")\n"));
    }

    #[cfg(feature = "ron")]
    #[test]
    fn ron_parses_without_the_extension_line() {
        let text = PINASG_RON
            .strip_prefix("#![enable(implicit_some)]\n")
            .unwrap();
        let asg: PinAsg = from_str(text, Format::Ron).unwrap();
        assert_eq!(
            asg.peripherals["USART2"].signals["RX"].bias,
            Some(Bias::PullUp)
        );
    }

    #[cfg(all(feature = "json", feature = "ron"))]
    #[test]
    fn formats_parse_to_equal_values() {
        let json: PinDef = from_str(PINDEF_JSON, Format::Json).unwrap();
        let ron: PinDef = from_str(PINDEF_RON, Format::Ron).unwrap();
        assert_eq!(json, ron);
    }

    #[cfg(feature = "ron")]
    #[test]
    fn unknown_field_is_rejected() {
        let text = PINASG_RON.replace("bias: pull_up", "pul: up");
        let err = from_str::<PinAsg>(&text, Format::Ron).unwrap_err();
        assert_eq!(err.code(), "PM0006");
        assert!(err.to_string().contains("pul"), "{err}");
    }

    #[cfg(feature = "ron")]
    #[test]
    fn wrong_kind_is_reported_as_such() {
        let err = from_str::<PinDef>(PINASG_RON, Format::Ron).unwrap_err();
        assert_eq!(err.code(), "PM0002");
    }

    #[cfg(feature = "json")]
    #[test]
    fn unsupported_schema_is_reported_as_such() {
        let text = PINDEF_JSON.replace("pinmap/v1", "pinmap/v9");
        let err = from_str::<PinDef>(&text, Format::Json).unwrap_err();
        assert_eq!(err.code(), "PM0003");
    }

    #[cfg(feature = "json")]
    #[test]
    fn read_rejects_a_file_named_as_the_other_kind() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("chip.pinasg.json");
        fs::write(&path, PINDEF_JSON).unwrap();
        let err = read::<PinDef>(&path).unwrap_err();
        assert_eq!(err.code(), "PM0002");
        assert!(err.to_string().contains("chip.pinasg.json"), "{err}");
    }

    #[cfg(feature = "json")]
    #[test]
    fn read_rejects_a_kind_field_that_disagrees_with_the_name() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("chip.pindef.json");
        fs::write(&path, PINDEF_JSON.replace(r#""pindef""#, r#""pinasg""#)).unwrap();
        let err = read::<PinDef>(&path).unwrap_err();
        assert_eq!(err.code(), "PM0002");
    }

    #[cfg(feature = "json")]
    #[test]
    fn write_then_read_gives_the_same_document() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("chip.pindef.json");
        let def: PinDef = from_str(PINDEF_JSON, Format::Json).unwrap();
        write(&path, &def).unwrap();
        assert_eq!(read::<PinDef>(&path).unwrap(), def);
    }
}
