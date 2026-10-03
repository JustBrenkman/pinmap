mod common;

use std::fs;

use common::{fixtures, pinmap};

#[test]
fn pin_list() {
    let run = pinmap(&fixtures().join("json"), &["pin", "list"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    insta::assert_snapshot!(run.stdout);
}

#[test]
fn pin_list_free() {
    let run = pinmap(
        &fixtures().join("check/diamond"),
        &["pin", "list", "--free"],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    insta::assert_snapshot!(run.stdout);
}

#[test]
fn pin_list_can() {
    let run = pinmap(
        &fixtures().join("json"),
        &["pin", "list", "--can", "TIM2.CH1"],
    );
    assert_eq!(run.code, 0);
    assert_eq!(
        run.stdout,
        "PIN  PAD  USE       NET      MUX\nPA5  30   SPI1.SCK  IMU_SCK  AF1\n"
    );
}

#[test]
fn pin_list_net_and_json() {
    let args = ["pin", "list", "--net", "IMU_CS", "--json"];
    let run = pinmap(&fixtures().join("json"), &args);
    let value: serde_json::Value = serde_json::from_str(&run.stdout).unwrap();
    assert_eq!(value["pins"][0]["pin"], "PA4");
    assert_eq!(value["pins"][0]["uses"][0]["kind"], "pin");
    assert_eq!(value["pins"].as_array().unwrap().len(), 1);
}

#[test]
fn pin_list_no_match() {
    let run = pinmap(&fixtures().join("json"), &["pin", "list", "--free"]);
    assert_eq!(run.stdout, "no pins match\n");
}

#[test]
fn bad_can_is_exit_2() {
    let run = pinmap(&fixtures().join("json"), &["pin", "list", "--can", "SPI1"]);
    assert_eq!(run.code, 2);
    assert!(
        run.stderr.contains("expected PERIPH.SIGNAL"),
        "{}",
        run.stderr
    );
}

#[test]
fn pin_show() {
    let run = pinmap(&fixtures().join("json"), &["pin", "show", "PA5"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    insta::assert_snapshot!(run.stdout);
}

#[test]
fn pin_show_imported_origin() {
    let run = pinmap(&fixtures().join("json"), &["pin", "show", "PA11"]);
    let expected = "origin  board/usb.pinasg.json  peripherals.OTG_FS.signals.DM";
    assert!(run.stdout.contains(expected), "{}", run.stdout);
}

#[test]
fn pin_show_suggests_case() {
    let run = pinmap(&fixtures().join("json"), &["pin", "show", "pa5"]);
    assert_eq!(run.code, 2);
    assert!(run.stderr.contains("did you mean PA5?"), "{}", run.stderr);
}

#[test]
fn pin_list_on_a_project_with_errors_notes_them() {
    let args = [
        "--project",
        "check/PM0102/board.pinasg.json",
        "pin",
        "list",
        "--used",
    ];
    let run = pinmap(&fixtures(), &args);
    assert_eq!(run.code, 0);
    assert!(
        run.stderr.contains("project has 1 error(s)"),
        "{}",
        run.stderr
    );
}

#[test]
fn pin_list_without_device_is_exit_2() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("board.pinasg.json"),
        r#"{"schema": "pinmap/v1", "kind": "pinasg", "device": "chip", "target": {"package": "P"}}"#,
    )
    .unwrap();
    let run = pinmap(dir.path(), &["pin", "list"]);
    assert_eq!(run.code, 2);
    assert!(run.stderr.contains("error[PM0005]"), "{}", run.stderr);
    assert!(
        run.stderr.contains("cannot load the project's pindef"),
        "{}",
        run.stderr
    );
}
