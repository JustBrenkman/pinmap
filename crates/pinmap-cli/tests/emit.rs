mod common;

use std::fs;
use std::path::Path;

use common::{fixtures, pinmap};

/// A temp copy of the fixture project, with the board and devices folders.
fn copy_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for sub in ["devices", "board"] {
        fs::create_dir_all(dir.path().join(sub)).unwrap();
        for entry in fs::read_dir(fixtures().join("json").join(sub)).unwrap() {
            let path = entry.unwrap().path();
            fs::copy(&path, dir.path().join(sub).join(path.file_name().unwrap())).unwrap();
        }
    }
    dir
}

#[test]
fn emit_markdown_to_stdout() {
    let run = pinmap(&fixtures().join("json"), &["emit", "--to", "markdown"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(
        run.stdout
            .starts_with("# STM32F407VGTx pinout (LQFP100, U1)\n"),
        "{}",
        run.stdout
    );
}

#[test]
fn output_file_creates_directories() {
    let dir = copy_project();
    let args = ["emit", "--to", "json", "-o", "out/deep/pins.json"];
    let run = pinmap(dir.path(), &args);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.stdout, "wrote out/deep/pins.json\n");
    let text = fs::read_to_string(dir.path().join("out/deep/pins.json")).unwrap();
    let rows: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(rows[0]["pin"], "PA2");
}

#[test]
fn out_dir_writes_named_files() {
    let dir = copy_project();
    let run = pinmap(
        dir.path(),
        &["emit", "--to", "markdown", "--out-dir", "out"],
    );
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(Path::new(&dir.path().join("out/pinout.md")).is_file());
    assert_eq!(run.stdout, "wrote out/pinout.md\n");
}

#[test]
fn errors_refuse_to_emit() {
    let args = [
        "--project",
        "check/PM0102/board.pinasg.json",
        "emit",
        "--to",
        "markdown",
    ];
    let run = pinmap(&fixtures(), &args);
    assert_eq!(run.code, 1);
    assert_eq!(run.stdout, "");
    assert!(run.stderr.contains("error[PM0102]"), "{}", run.stderr);
    assert!(run.stderr.contains("--allow-errors"), "{}", run.stderr);

    let mut args = args.to_vec();
    args.push("--allow-errors");
    assert_eq!(pinmap(&fixtures(), &args).code, 0);
}

#[test]
fn unknown_emitter_is_exit_2() {
    let run = pinmap(&fixtures().join("json"), &["emit", "--to", "zephyr"]);
    assert_eq!(run.code, 2);
    assert!(run.stderr.contains("markdown"), "{}", run.stderr);
    assert!(run.stderr.contains("json"), "{}", run.stderr);
}

#[test]
fn unknown_option_is_exit_2() {
    let json = fixtures().join("json");
    let run = pinmap(&json, &["emit", "--to", "markdown", "-O", "freepins=true"]);
    assert_eq!(run.code, 2);
    assert!(run.stderr.contains("options: free-pins"), "{}", run.stderr);
    let run = pinmap(&json, &["emit", "--to", "markdown", "-O", "free-pins=yes"]);
    assert_eq!(run.code, 2);
    let run = pinmap(&json, &["emit", "--to", "markdown", "-O", "nokey"]);
    assert_eq!(run.code, 2);
    assert!(run.stderr.contains("expected KEY=VALUE"), "{}", run.stderr);
}

#[test]
fn list_emitters() {
    let run = pinmap(&fixtures(), &["list", "emitters"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    insta::assert_snapshot!(run.stdout);
    let run = pinmap(&fixtures(), &["list", "emitters", "--json"]);
    let value: serde_json::Value = serde_json::from_str(&run.stdout).unwrap();
    assert_eq!(value["emitters"][0]["id"], "markdown");
    assert_eq!(value["emitters"][0]["options"][0]["name"], "free-pins");
}

#[cfg(unix)]
#[test]
fn errors_name_their_cause_once() {
    let dir = copy_project();
    fs::create_dir(dir.path().join("taken")).unwrap();
    let run = pinmap(dir.path(), &["emit", "--to", "json", "-o", "taken"]);
    assert_eq!(run.code, 2);
    assert_eq!(
        run.stderr.matches("Is a directory").count(),
        1,
        "{}",
        run.stderr
    );
}
