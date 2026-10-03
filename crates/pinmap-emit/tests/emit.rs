use std::fs;
use std::path::{Path, PathBuf};

use pinmap_core::{EmitOptions, check_options, open};

fn fixtures() -> PathBuf {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    crates.parent().unwrap().join("fixtures")
}

/// Runs emitter `id` on the project rooted at `root` and returns its one file.
fn emit(id: &str, root: &Path, options: &[(&str, &str)]) -> String {
    let (resolved, _) = open(root);
    let resolved = resolved.expect("project loads");
    let emitters = pinmap_emit::all();
    let emitter = emitters
        .iter()
        .find(|e| e.id() == id)
        .unwrap_or_else(|| panic!("no emitter {id}"));
    let options: EmitOptions = options
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    check_options(emitter.as_ref(), &options).unwrap();
    let output = emitter.emit(&resolved, &options).unwrap();
    assert_eq!(output.files.len(), 1);
    output.files.into_iter().next().unwrap().contents
}

#[test]
fn markdown_pinout() {
    let root = fixtures().join("json/board/revb.pinasg.json");
    insta::assert_snapshot!(emit("markdown", &root, &[]));
}

#[test]
fn markdown_free_pins() {
    let root = fixtures().join("check/diamond/root.pinasg.json");
    assert!(!emit("markdown", &root, &[]).contains("| PA5 | 30 |"));
    let text = emit("markdown", &root, &[("free-pins", "true")]);
    assert!(text.contains("| PA5 | 30 |"), "{text}");
}

#[test]
fn cells_are_escaped() {
    let dir = tempfile::tempdir().unwrap();
    fs::copy(
        fixtures().join("json/devices/stm32f407vg.pindef.json"),
        dir.path().join("chip.pindef.json"),
    )
    .unwrap();
    let root = dir.path().join("board.pinasg.json");
    fs::write(
        &root,
        r#"{"schema": "pinmap/v1", "kind": "pinasg", "device": "chip",
            "target": {"package": "LQFP100"},
            "pins": {"PA4": {"mode": "gpio_out", "note": "a | b\nc"}}}"#,
    )
    .unwrap();
    let text = emit("markdown", &root, &[]);
    assert!(text.contains("a \\| b c"), "{text}");
}
