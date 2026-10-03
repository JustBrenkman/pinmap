use std::fs;
use std::path::{Path, PathBuf};

use pinmap_core::{Diagnostic, load};

fn fixtures() -> PathBuf {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    crates.parent().unwrap().join("fixtures")
}

/// `"CODE file-name logical-path"` per diagnostic, `-` for a missing part.
fn summary(diags: &[Diagnostic]) -> Vec<String> {
    diags
        .iter()
        .map(|d| {
            let file = d
                .file
                .as_ref()
                .and_then(|f| f.file_name())
                .map(|n| n.to_string_lossy().into_owned());
            let path = d.path.as_deref().unwrap_or("-");
            format!("{} {} {path}", d.code, file.as_deref().unwrap_or("-"))
        })
        .collect()
}

#[test]
fn loads_root_imports_and_device() {
    let root = fixtures().join("json/board/revb.pinasg.json");
    let (project, diags) = load(&root);
    assert!(diags.is_empty(), "{diags:?}");
    let files: Vec<&Path> = project.files.iter().map(|f| f.path.as_path()).collect();
    assert_eq!(
        files,
        [root.clone(), fixtures().join("json/board/usb.pinasg.json")]
    );
    let device = project.device.unwrap();
    assert_eq!(
        device.path,
        fixtures().join("json/devices/stm32f407vg.pindef.json")
    );
    assert_eq!(device.doc.device.name, "STM32F407VGTx");
}

#[test]
fn imported_paths_are_normalized() {
    let (project, _) = load(Path::new("../../fixtures/json/board/revb.pinasg.json"));
    assert_eq!(project.files.len(), 2);
    for file in &project.files {
        let shown = file.path.to_string_lossy();
        assert!(shown.starts_with("../../fixtures/json/board/"), "{shown}");
    }
    let device = project.device.unwrap().path;
    assert_eq!(
        device,
        Path::new("../../fixtures/json/devices/stm32f407vg.pindef.json")
    );
}

#[test]
fn diamond_imports_load_once() {
    let (project, diags) = load(&fixtures().join("check/diamond/root.pinasg.json"));
    assert!(diags.is_empty(), "{diags:?}");
    let names: Vec<String> = project
        .files
        .iter()
        .map(|f| f.path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        names,
        [
            "root.pinasg.json",
            "a.pinasg.json",
            "shared.pinasg.json",
            "b.pinasg.json"
        ]
    );
}

#[test]
fn cycle_is_reported_at_the_import_that_closes_it() {
    let (project, diags) = load(&fixtures().join("check/PM0107/board.pinasg.json"));
    assert_eq!(summary(&diags), ["PM0107 loop.pinasg.json imports[0]"]);
    assert!(
        diags[0].message.contains("board.pinasg.json -> "),
        "{}",
        diags[0].message
    );
    assert_eq!(project.files.len(), 2);
}

#[test]
fn import_must_match_root_device_and_target() {
    let (_, diags) = load(&fixtures().join("check/PM0109/board.pinasg.json"));
    assert_eq!(
        summary(&diags),
        [
            "PM0109 other.pinasg.json device",
            "PM0109 other.pinasg.json target"
        ]
    );
}

#[test]
fn missing_root_is_a_diagnostic() {
    let (project, diags) = load(Path::new("nowhere/board.pinasg.json"));
    assert_eq!(summary(&diags), ["PM0008 board.pinasg.json -"]);
    assert!(project.device.is_none());
    assert!(project.files.is_empty());
}

#[test]
fn missing_import_and_device_are_reported_where_referenced() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("board.pinasg.json");
    fs::write(
        &root,
        r#"{"schema": "pinmap/v1", "kind": "pinasg", "device": "nope",
            "target": {"package": "LQFP100"}, "imports": ["gone"]}"#,
    )
    .unwrap();
    let (project, diags) = load(&root);
    assert_eq!(
        summary(&diags),
        [
            "PM0005 board.pinasg.json device",
            "PM0005 board.pinasg.json imports[0]"
        ]
    );
    assert!(project.device.is_none());
    assert_eq!(project.files.len(), 1);
}

#[test]
fn reference_errors_show_normalized_paths() {
    let dir = tempfile::tempdir().unwrap();
    let board = dir.path().join("board");
    fs::create_dir(&board).unwrap();
    fs::write(
        board.join("main.pinasg.json"),
        r#"{"schema": "pinmap/v1", "kind": "pinasg", "device": "../devices/chip",
            "target": {"package": "P"}}"#,
    )
    .unwrap();
    let (_, diags) = load(&board.join("main.pinasg.json"));
    assert_eq!(diags[0].code, "PM0005");
    assert!(
        diags[0].message.contains("devices/chip.pindef.ron"),
        "{}",
        diags[0].message
    );
    // The reference is echoed as written; the paths pinmap built are clean.
    let (_, tried) = diags[0].message.split_once("looked for").unwrap();
    assert!(!tried.contains(".."), "{}", diags[0].message);
}
