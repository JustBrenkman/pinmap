//! Every JSON fixture must validate against the generated JSON Schema.

#![cfg(all(feature = "schema", feature = "json"))]

use std::fs;
use std::path::{Path, PathBuf};

use pinmap_model::{DocKind, Format, classify, json_schema};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn json_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            json_files(&path, out);
        } else if matches!(classify(&path), Ok((_, Format::Json))) {
            out.push(path);
        }
    }
}

fn errors(kind: DocKind, instance: &serde_json::Value) -> Vec<String> {
    let schema = serde_json::to_value(json_schema(kind)).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    validator
        .iter_errors(instance)
        .map(|e| e.to_string())
        .collect()
}

fn load(path: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn every_json_fixture_matches_its_schema() {
    let mut files = Vec::new();
    json_files(&fixtures(), &mut files);
    assert!(files.len() >= 3, "found {files:?}");
    for file in files {
        let (kind, _) = classify(&file).unwrap();
        let errors = errors(kind, &load(&file));
        assert!(errors.is_empty(), "{}: {errors:?}", file.display());
    }
}

#[test]
fn schema_rejects_unknown_fields() {
    let mut instance = load(&fixtures().join("json/board/usb.pinasg.json"));
    instance["peripherals"]["OTG_FS"]["signals"]["DM"]["pul"] = "up".into();
    assert!(!errors(DocKind::Pinasg, &instance).is_empty());
}

#[test]
fn schema_rejects_bad_enum_values() {
    let mut instance = load(&fixtures().join("json/board/usb.pinasg.json"));
    instance["peripherals"]["OTG_FS"]["signals"]["DM"]["bias"] = "none".into();
    assert!(!errors(DocKind::Pinasg, &instance).is_empty());
}
