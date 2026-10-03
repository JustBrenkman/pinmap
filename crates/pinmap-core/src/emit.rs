use std::path::PathBuf;

use indexmap::IndexMap;

use crate::{Diagnostic, Resolved};

/// `-O key=value` options passed to an emitter.
pub type EmitOptions = IndexMap<String, String>;

/// An option an emitter accepts.
pub struct OptionSpec {
    pub name: &'static str,
    pub help: &'static str,
    pub default: &'static str,
}

/// A generated file. `path` is relative to the output directory.
pub struct OutputFile {
    pub path: PathBuf,
    pub contents: String,
}

#[derive(Default)]
pub struct EmitOutput {
    pub files: Vec<OutputFile>,
    /// Parts of the project the output format cannot express.
    pub unsupported: Vec<Diagnostic>,
}

#[derive(Debug, thiserror::Error)]
pub enum EmitError {
    #[error("{emitter} has no option {key}; options: {}", options_text(.known))]
    UnknownOption {
        emitter: &'static str,
        key: String,
        known: Vec<&'static str>,
    },
    #[error("option {key} expects {expected}, got {value:?}")]
    BadOption {
        key: String,
        value: String,
        expected: &'static str,
    },
}

fn options_text(known: &[&str]) -> String {
    if known.is_empty() {
        "none".into()
    } else {
        known.join(", ")
    }
}

/// Turns a resolved project into output files. Emitters build files in
/// memory and never write; the caller decides where output goes.
pub trait Emitter {
    fn id(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn options(&self) -> &'static [OptionSpec];
    fn emit(&self, r: &Resolved, options: &EmitOptions) -> Result<EmitOutput, EmitError>;
}

/// Rejects options the emitter does not declare.
pub fn check_options(emitter: &dyn Emitter, options: &EmitOptions) -> Result<(), EmitError> {
    let known: Vec<&'static str> = emitter.options().iter().map(|o| o.name).collect();
    match options.keys().find(|key| !known.contains(&key.as_str())) {
        Some(key) => Err(EmitError::UnknownOption {
            emitter: emitter.id(),
            key: key.clone(),
            known,
        }),
        None => Ok(()),
    }
}

/// Reads a `true`/`false` option, falling back to its declared default.
pub fn bool_option(
    emitter: &dyn Emitter,
    options: &EmitOptions,
    name: &str,
) -> Result<bool, EmitError> {
    let default = emitter
        .options()
        .iter()
        .find(|o| o.name == name)
        .map_or("false", |o| o.default);
    let value = options.get(name).map_or(default, String::as_str);
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(EmitError::BadOption {
            key: name.into(),
            value: value.into(),
            expected: "true or false",
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dummy;

    impl Emitter for Dummy {
        fn id(&self) -> &'static str {
            "dummy"
        }
        fn description(&self) -> &'static str {
            "test"
        }
        fn options(&self) -> &'static [OptionSpec] {
            &[OptionSpec {
                name: "free-pins",
                help: "",
                default: "false",
            }]
        }
        fn emit(&self, _: &Resolved, _: &EmitOptions) -> Result<EmitOutput, EmitError> {
            Ok(EmitOutput::default())
        }
    }

    fn options(pairs: &[(&str, &str)]) -> EmitOptions {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn unknown_options_are_rejected() {
        assert!(check_options(&Dummy, &options(&[])).is_ok());
        assert!(check_options(&Dummy, &options(&[("free-pins", "true")])).is_ok());
        let err = check_options(&Dummy, &options(&[("freepins", "true")])).unwrap_err();
        assert_eq!(
            err.to_string(),
            "dummy has no option freepins; options: free-pins"
        );
    }

    #[test]
    fn bool_options_use_their_default() {
        assert!(!bool_option(&Dummy, &options(&[]), "free-pins").unwrap());
        assert!(bool_option(&Dummy, &options(&[("free-pins", "true")]), "free-pins").unwrap());
        assert!(!bool_option(&Dummy, &options(&[("free-pins", "false")]), "free-pins").unwrap());
        let err = bool_option(&Dummy, &options(&[("free-pins", "yes")]), "free-pins").unwrap_err();
        assert!(matches!(err, EmitError::BadOption { .. }));
    }
}
