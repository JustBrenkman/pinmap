use std::fs;
use std::path::{Path, PathBuf};

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
    read_from(&path, &path)
}

/// Reads a document whose kind and format come from the file name `named`
/// and whose contents come from `disk`. They differ when `named` is a
/// symlink to a file with another name.
pub fn read_from<T: Document>(
    named: impl AsRef<Path>,
    disk: impl AsRef<Path>,
) -> Result<T, ModelError> {
    let named = named.as_ref();
    let load = || {
        let format = classify_as::<T>(named)?;
        from_str(&fs::read_to_string(disk)?, format)
    };
    load().map_err(|err| at(err, named))
}

/// Writes a document to a file in canonical form. The file name decides the
/// format and must name the kind that `T` represents.
pub fn write<T: Document>(path: impl AsRef<Path>, doc: &T) -> Result<(), ModelError> {
    let path = path.as_ref();
    let render = || to_string(doc, classify_as::<T>(path)?);
    let text = render().map_err(|err| at(err, path))?;
    write_atomic(path, &text)
}

/// Writes `contents` to `path` through a temporary file in the same
/// directory and a rename, so a reader never sees a half-written file.
pub fn write_atomic(path: impl AsRef<Path>, contents: &str) -> Result<(), ModelError> {
    stage(path, contents)?.commit()
}

/// A file written to its temporary name but not yet renamed into place.
/// Dropping it without [`Staged::commit`] deletes the temporary file, so
/// several files can be staged first and committed only if all staged.
pub struct Staged {
    temp: PathBuf,
    path: PathBuf,
    done: bool,
}

/// Writes `contents` next to `path` under a temporary name.
pub fn stage(path: impl AsRef<Path>, contents: &str) -> Result<Staged, ModelError> {
    let path = path.as_ref();
    let name = path.file_name().ok_or_else(|| {
        let err = std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no file name");
        ModelError::from(err).at(path)
    })?;
    let mut temp_name = std::ffi::OsString::from(".");
    temp_name.push(name);
    temp_name.push(".pinmap-tmp");
    let mut staged = Staged {
        temp: path.with_file_name(temp_name),
        path: path.to_path_buf(),
        done: false,
    };
    if let Err(err) = fs::write(&staged.temp, contents) {
        // Nothing to clean up if the file was never created.
        staged.done = !staged.temp.exists();
        return Err(ModelError::from(err).at(path));
    }
    Ok(staged)
}

impl Staged {
    /// Renames the temporary file over the target.
    pub fn commit(mut self) -> Result<(), ModelError> {
        let result = fs::rename(&self.temp, &self.path);
        self.done = result.is_ok();
        result.map_err(|err| ModelError::from(err).at(&self.path))
    }
}

impl Drop for Staged {
    fn drop(&mut self) {
        if !self.done {
            let _ = fs::remove_file(&self.temp);
        }
    }
}

/// Either kind of document, for code that handles both, such as conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnyDoc {
    Pindef(PinDef),
    Pinasg(PinAsg),
}

impl AnyDoc {
    pub fn kind(&self) -> DocKind {
        match self {
            Self::Pindef(_) => DocKind::Pindef,
            Self::Pinasg(_) => DocKind::Pinasg,
        }
    }

    /// Parses a document whose kind is known only at run time.
    pub fn from_str(text: &str, kind: DocKind, format: Format) -> Result<Self, ModelError> {
        Ok(match kind {
            DocKind::Pindef => Self::Pindef(from_str(text, format)?),
            DocKind::Pinasg => Self::Pinasg(from_str(text, format)?),
        })
    }

    /// Serializes to canonical text, as [`to_string`] does.
    pub fn to_string(&self, format: Format) -> Result<String, ModelError> {
        match self {
            Self::Pindef(doc) => to_string(doc, format),
            Self::Pinasg(doc) => to_string(doc, format),
        }
    }
}

/// Reads a document of either kind. The file name decides kind and format.
pub fn read_any(path: impl AsRef<Path>) -> Result<AnyDoc, ModelError> {
    let path = path.as_ref();
    let load = || {
        let (kind, format) = classify(path)?;
        let text = fs::read_to_string(path)?;
        AnyDoc::from_str(&text, kind, format)
    };
    load().map_err(|err| at(err, path))
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
    expect_kind::<T>(kind)?;
    Ok(format)
}

fn check_header<T: Document>(schema: &SchemaVersion, kind: DocKind) -> Result<(), ModelError> {
    if !schema.is_supported() {
        return Err(ModelError::UnsupportedSchema {
            found: schema.as_str().to_owned(),
        });
    }
    expect_kind::<T>(kind)
}

fn expect_kind<T: Document>(found: DocKind) -> Result<(), ModelError> {
    if found == T::KIND {
        Ok(())
    } else {
        Err(ModelError::KindMismatch {
            expected: T::KIND,
            found,
        })
    }
}

fn parse<T: DeserializeOwned>(text: &str, format: Format) -> Result<T, ModelError> {
    let parsed: Result<T, String> = match format {
        #[cfg(feature = "json")]
        Format::Json => serde_json::from_str(text).map_err(|e| e.to_string()),
        #[cfg(feature = "ron")]
        Format::Ron => ron::Options::default()
            // Accept files with or without the `#![enable(implicit_some)]` line.
            .with_default_extension(ron::extensions::Extensions::IMPLICIT_SOME)
            .from_str(text)
            .map_err(|e| e.to_string()),
        #[allow(unreachable_patterns)]
        _ => return Err(ModelError::FormatDisabled { format }),
    };
    parsed.map_err(|message| ModelError::Parse { format, message })
}

fn print<T: Serialize>(doc: &T, format: Format) -> Result<String, ModelError> {
    let printed: Result<String, String> = match format {
        #[cfg(feature = "json")]
        Format::Json => serde_json::to_string_pretty(doc).map_err(|e| e.to_string()),
        #[cfg(feature = "ron")]
        Format::Ron => {
            let config = ron::ser::PrettyConfig::new()
                .new_line("\n")
                .indentor("    ")
                .struct_names(false)
                .extensions(ron::extensions::Extensions::IMPLICIT_SOME);
            ron::ser::to_string_pretty(doc, config).map_err(|e| e.to_string())
        }
        #[allow(unreachable_patterns)]
        _ => return Err(ModelError::FormatDisabled { format }),
    };
    printed.map_err(|message| ModelError::Parse { format, message })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_atomic_replaces_contents_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.pinasg.json");
        fs::write(&path, "old").unwrap();
        write_atomic(&path, "new").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
        let names: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["x.pinasg.json"]);
    }

    #[test]
    fn failed_rename_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        // A directory where the file should go makes the rename fail.
        let path = dir.path().join("x.pinasg.json");
        fs::create_dir(&path).unwrap();
        fs::write(path.join("keep"), "").unwrap();
        let err = write_atomic(&path, "new").unwrap_err();
        assert_eq!(err.code(), "PM0008");
        let names: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["x.pinasg.json"]);
    }
    #[cfg(feature = "ron")]
    use crate::{Bias, PinAsg};
    use crate::{Mux, PinDef};

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
