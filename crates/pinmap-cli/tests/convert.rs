mod common;

use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

use common::{fixtures, pinmap};

fn copy_fixture(dir: &tempfile::TempDir, from: &str, to: &str) {
    fs::copy(fixtures().join(from), dir.path().join(to)).unwrap();
}

#[test]
fn json_to_ron_matches_the_ron_fixture() {
    let dir = tempfile::tempdir().unwrap();
    copy_fixture(&dir, "json/board/usb.pinasg.json", "usb.pinasg.json");
    let run = pinmap(
        dir.path(),
        &["convert", "usb.pinasg.json", "usb.pinasg.ron"],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    let written = fs::read_to_string(dir.path().join("usb.pinasg.ron")).unwrap();
    let expected = fs::read_to_string(fixtures().join("ron/board/usb.pinasg.ron")).unwrap();
    assert_eq!(written, expected);
    assert!(run.stdout.contains("stem"), "{}", run.stdout);
}

#[test]
fn rm_source_removes_the_input() {
    let dir = tempfile::tempdir().unwrap();
    copy_fixture(&dir, "json/board/usb.pinasg.json", "usb.pinasg.json");
    let args = [
        "convert",
        "usb.pinasg.json",
        "usb.pinasg.ron",
        "--rm-source",
    ];
    let run = pinmap(dir.path(), &args);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(!dir.path().join("usb.pinasg.json").exists());
}

#[test]
fn rm_source_refuses_commented_ron() {
    let dir = tempfile::tempdir().unwrap();
    let ron = fs::read_to_string(fixtures().join("ron/board/usb.pinasg.ron")).unwrap();
    fs::write(
        dir.path().join("usb.pinasg.ron"),
        format!("// keep me\n{ron}"),
    )
    .unwrap();
    let args = [
        "convert",
        "usb.pinasg.ron",
        "usb.pinasg.json",
        "--rm-source",
    ];
    let run = pinmap(dir.path(), &args);
    assert_eq!(run.code, 2);
    assert!(run.stderr.contains("comments"), "{}", run.stderr);
    assert!(dir.path().join("usb.pinasg.ron").exists());
    assert!(!dir.path().join("usb.pinasg.json").exists());
}

#[test]
fn refuses_to_overwrite_without_force() {
    let dir = tempfile::tempdir().unwrap();
    copy_fixture(&dir, "json/board/usb.pinasg.json", "usb.pinasg.json");
    copy_fixture(&dir, "ron/board/usb.pinasg.ron", "usb.pinasg.ron");
    let run = pinmap(
        dir.path(),
        &["convert", "usb.pinasg.json", "usb.pinasg.ron"],
    );
    assert_eq!(run.code, 2);
    assert!(run.stderr.contains("--force"), "{}", run.stderr);
    let args = ["convert", "usb.pinasg.json", "usb.pinasg.ron", "--force"];
    let run = pinmap(dir.path(), &args);
    assert_eq!(run.code, 0, "{}", run.stderr);
}

#[test]
fn kinds_must_match() {
    let dir = tempfile::tempdir().unwrap();
    copy_fixture(&dir, "json/board/usb.pinasg.json", "usb.pinasg.json");
    let run = pinmap(
        dir.path(),
        &["convert", "usb.pinasg.json", "usb.pindef.ron"],
    );
    assert_eq!(run.code, 2);
    let expected = "cannot convert a pinasg into a pindef";
    assert!(run.stderr.contains(expected), "{}", run.stderr);
}

#[test]
fn stdin_to_stdout() {
    let json = fs::read_to_string(fixtures().join("json/board/usb.pinasg.json")).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_pinmap"))
        .args([
            "convert", "-", "--kind", "pinasg", "--from", "json", "--to", "ron",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(json.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let expected = fs::read_to_string(fixtures().join("ron/board/usb.pinasg.ron")).unwrap();
    assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
}

#[test]
fn stdin_needs_kind_and_formats() {
    let run = pinmap(&fixtures(), &["convert", "-", "--kind", "pinasg"]);
    assert_eq!(run.code, 2);
    assert!(
        run.stderr.contains("--kind, --from and --to"),
        "{}",
        run.stderr
    );
}
