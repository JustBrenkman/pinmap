mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{fixtures, pinmap};

fn copy_tree(from: &Path, to: &Path) {
    for entry in fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let target = to.join(path.file_name().unwrap());
        if path.is_dir() {
            fs::create_dir_all(&target).unwrap();
            copy_tree(&path, &target);
        } else {
            fs::copy(&path, &target).unwrap();
        }
    }
}

fn documents(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            documents(&path, out);
        } else if pinmap_model::classify(&path).is_ok() {
            out.push(path);
        }
    }
}

#[test]
fn every_fixture_is_canonical() {
    let mut files = Vec::new();
    documents(&fixtures(), &mut files);
    let mut args = vec!["fmt".to_owned(), "--check".to_owned()];
    args.extend(files.iter().map(|f| f.to_string_lossy().into_owned()));
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let run = pinmap(&fixtures(), &args);
    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
}

#[test]
fn project_files_are_found_without_paths() {
    let dir = tempfile::tempdir().unwrap();
    copy_tree(&fixtures().join("json"), dir.path());
    let run = pinmap(dir.path(), &["fmt", "--check"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.stdout, "3 files checked, 0 need formatting\n");
}

#[test]
fn check_reports_and_fmt_rewrites() {
    let dir = tempfile::tempdir().unwrap();
    copy_tree(&fixtures().join("json"), dir.path());
    let usb = dir.path().join("board/usb.pinasg.json");
    let canonical = fs::read_to_string(&usb).unwrap();
    let compact: serde_json::Value = serde_json::from_str(&canonical).unwrap();
    fs::write(&usb, compact.to_string()).unwrap();

    let run = pinmap(dir.path(), &["fmt", "--check"]);
    assert_eq!(run.code, 1);
    let expected = "would reformat board/usb.pinasg.json";
    assert!(run.stdout.contains(expected), "{}", run.stdout);

    let run = pinmap(dir.path(), &["fmt"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let expected = "reformatted board/usb.pinasg.json";
    assert!(run.stdout.contains(expected), "{}", run.stdout);
    assert_eq!(fs::read_to_string(&usb).unwrap(), canonical);
}

#[test]
fn fmt_skips_commented_ron() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("usb.pinasg.ron");
    let ron = fs::read_to_string(fixtures().join("ron/board/usb.pinasg.ron")).unwrap();
    let commented = ron.replacen("(\n", "( // USB port\n", 1);
    fs::write(&path, &commented).unwrap();

    let run = pinmap(dir.path(), &["fmt", "usb.pinasg.ron"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let expected = "skipped usb.pinasg.ron: has comments";
    assert!(run.stdout.contains(expected), "{}", run.stdout);
    assert_eq!(fs::read_to_string(&path).unwrap(), commented);
}

#[test]
fn unparseable_file_is_a_problem() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("bad.pinasg.json"), "{").unwrap();
    let run = pinmap(dir.path(), &["fmt", "bad.pinasg.json"]);
    assert_eq!(run.code, 1);
    assert!(
        run.stdout.contains("error: bad.pinasg.json"),
        "{}",
        run.stdout
    );
}
