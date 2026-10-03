mod common;

use std::fs;

use common::{fixtures, pinmap};

#[test]
fn clean_project_found_by_discovery() {
    let run = pinmap(&fixtures().join("json"), &["check"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.stdout, "✔ no problems\n");
}

#[test]
fn errors_print_in_design_style() {
    let args = ["--project", "check/PM0102/board.pinasg.json", "check"];
    let run = pinmap(&fixtures(), &args);
    assert_eq!(run.code, 1);
    insta::assert_snapshot!(run.stdout);
}

#[test]
fn json_output() {
    let args = [
        "check",
        "--json",
        "--project",
        "check/PM0109/board.pinasg.json",
    ];
    let run = pinmap(&fixtures(), &args);
    assert_eq!(run.code, 1);
    let value: serde_json::Value = serde_json::from_str(&run.stdout).unwrap();
    assert_eq!(value["summary"]["errors"], 2);
    insta::assert_snapshot!(run.stdout);
}

#[test]
fn info_does_not_fail() {
    let args = [
        "--project",
        "check/PM0110/board.pinasg.json",
        "check",
        "--deny-warnings",
    ];
    let run = pinmap(&fixtures(), &args);
    assert_eq!(run.code, 0);
    assert!(run.stdout.ends_with("✔ 1 info\n"), "{}", run.stdout);
}

#[test]
fn several_roots_is_exit_2() {
    let run = pinmap(&fixtures(), &["check"]);
    assert_eq!(run.code, 2);
    assert!(
        run.stderr.contains("choose one with --project"),
        "{}",
        run.stderr
    );
    assert!(
        run.stderr.contains("json/board/revb.pinasg.json"),
        "{}",
        run.stderr
    );
}

#[test]
fn missing_device_fails_check() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("board.pinasg.json"),
        r#"{"schema": "pinmap/v1", "kind": "pinasg", "device": "chip", "target": {"package": "P"}}"#,
    )
    .unwrap();
    let run = pinmap(dir.path(), &["check"]);
    assert_eq!(run.code, 1);
    assert!(run.stdout.contains("error[PM0005]"), "{}", run.stdout);
}
