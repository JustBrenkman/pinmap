use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::{DocKind, Format, ModelError, classify};

/// Probe order for stems. Finding both is an error, so the order only decides
/// how candidates are listed.
const PROBE: [Format; 2] = [Format::Ron, Format::Json];

/// Resolves a `device` or `imports` reference to a file.
///
/// `base_dir` is the directory of the referring file, and `kind` is the kind
/// the referring field implies (`device` is a pindef, `imports` are pinasgs).
///
/// - A reference with a full `.<kind>.<format>` ending names exactly that file.
/// - Anything else is a stem: `<stem>.<kind>.ron` and `<stem>.<kind>.json` are
///   probed. If both exist the reference is ambiguous, with no silent
///   preference. Stems let a file change format without edits to its referrers.
pub fn resolve_ref(
    base_dir: &Path,
    reference: &Path,
    kind: DocKind,
) -> Result<PathBuf, ModelError> {
    if let Ok((found, _)) = classify(reference) {
        if found != kind {
            let err = ModelError::KindMismatch {
                expected: kind,
                found,
            };
            return Err(err.at(reference));
        }
        let path = base_dir.join(reference);
        return if path.is_file() {
            Ok(path)
        } else {
            Err(ModelError::RefNotFound {
                reference: reference.to_path_buf(),
                tried: vec![path],
            })
        };
    }

    let [ron, json] = PROBE.map(|format| {
        let mut name = OsString::from(base_dir.join(reference));
        name.push(format!(".{kind}.{format}"));
        PathBuf::from(name)
    });
    match (ron.is_file(), json.is_file()) {
        (true, true) => Err(ModelError::AmbiguousRef {
            reference: reference.to_path_buf(),
            candidates: [ron, json],
        }),
        (true, false) => Ok(ron),
        (false, true) => Ok(json),
        (false, false) => Err(ModelError::RefNotFound {
            reference: reference.to_path_buf(),
            tried: vec![ron, json],
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn dir_with(files: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for file in files {
            let path = dir.path().join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "").unwrap();
        }
        dir
    }

    fn resolve(
        dir: &tempfile::TempDir,
        reference: &str,
        kind: DocKind,
    ) -> Result<PathBuf, ModelError> {
        resolve_ref(dir.path(), Path::new(reference), kind)
    }

    #[test]
    fn stem_finds_either_format() {
        let dir = dir_with(&["chip.pindef.json", "usb.pinasg.ron"]);
        assert_eq!(
            resolve(&dir, "chip", DocKind::Pindef).unwrap(),
            dir.path().join("chip.pindef.json")
        );
        assert_eq!(
            resolve(&dir, "usb", DocKind::Pinasg).unwrap(),
            dir.path().join("usb.pinasg.ron")
        );
    }

    #[test]
    fn stem_implies_the_kind() {
        let dir = dir_with(&["x.pindef.json", "x.pinasg.ron"]);
        assert_eq!(
            resolve(&dir, "x", DocKind::Pindef).unwrap(),
            dir.path().join("x.pindef.json")
        );
    }

    #[test]
    fn stem_may_contain_dots_and_directories() {
        let dir = dir_with(&["devices/stm32.f407.pindef.ron"]);
        assert_eq!(
            resolve(&dir, "devices/stm32.f407", DocKind::Pindef).unwrap(),
            dir.path().join("devices/stm32.f407.pindef.ron")
        );
    }

    #[test]
    fn stem_with_both_formats_is_ambiguous() {
        let dir = dir_with(&["chip.pindef.json", "chip.pindef.ron"]);
        let err = resolve(&dir, "chip", DocKind::Pindef).unwrap_err();
        assert_eq!(err.code(), "PM0004");
    }

    #[test]
    fn explicit_path_is_used_as_is() {
        let dir = dir_with(&["chip.pindef.json", "chip.pindef.ron"]);
        assert_eq!(
            resolve(&dir, "chip.pindef.json", DocKind::Pindef).unwrap(),
            dir.path().join("chip.pindef.json")
        );
    }

    #[test]
    fn explicit_path_of_the_wrong_kind_is_rejected() {
        let dir = dir_with(&["chip.pindef.json"]);
        let err = resolve(&dir, "chip.pindef.json", DocKind::Pinasg).unwrap_err();
        assert_eq!(err.code(), "PM0002");
    }

    #[test]
    fn missing_reference_lists_what_was_tried() {
        let dir = dir_with(&[]);
        let err = resolve(&dir, "chip", DocKind::Pindef).unwrap_err();
        assert_eq!(err.code(), "PM0005");
        let message = err.to_string();
        assert!(message.contains("chip.pindef.ron"), "{message}");
        assert!(message.contains("chip.pindef.json"), "{message}");

        let err = resolve(&dir, "chip.pindef.json", DocKind::Pindef).unwrap_err();
        assert_eq!(err.code(), "PM0005");
    }
}
