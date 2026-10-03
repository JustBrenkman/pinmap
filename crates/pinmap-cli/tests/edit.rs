mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{Run, fixtures, pinmap};

/// A temp project: the fixture pindef and a board file with only `device`
/// and `target`. Returns the directory and the board path.
fn empty_project() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("devices")).unwrap();
    fs::create_dir_all(dir.path().join("board")).unwrap();
    fs::copy(
        fixtures().join("json/devices/stm32f407vg.pindef.json"),
        dir.path().join("devices/stm32f407vg.pindef.json"),
    )
    .unwrap();
    let board = dir.path().join("board/board.pinasg.json");
    fs::write(
        &board,
        "{\n  \"schema\": \"pinmap/v1\",\n  \"kind\": \"pinasg\",\n  \"device\": \"../devices/stm32f407vg\",\n  \"target\": {\n    \"package\": \"LQFP100\",\n    \"refdes\": \"U1\"\n  }\n}\n",
    )
    .unwrap();
    (dir, board)
}

fn run(dir: &Path, args: &[&str]) -> Run {
    pinmap(dir, args)
}

#[test]
fn assign_periph_writes() {
    let (dir, board) = empty_project();
    let args = [
        "assign",
        "periph",
        "SPI1",
        "SCK=PA5:IMU_SCK",
        "MISO=PA6",
        "--speed",
        "very-high",
    ];
    let r = run(dir.path(), &args);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert_eq!(
        r.stdout,
        "updated board/board.pinasg.json\n✔ updated 1 file\n"
    );
    let text = fs::read_to_string(&board).unwrap();
    let expected = "\"MISO\": {\n          \"pin\": \"PA6\",\n          \"speed\": \"very_high\"";
    assert!(text.contains(expected), "{text}");
}

#[test]
fn dry_run_shows_a_diff() {
    let (dir, board) = empty_project();
    let before = fs::read_to_string(&board).unwrap();
    let r = run(
        dir.path(),
        &["assign", "periph", "SPI1", "SCK=PA5:IMU_SCK", "--dry-run"],
    );
    assert_eq!(r.code, 0, "{}", r.stderr);
    insta::assert_snapshot!(r.stdout);
    assert_eq!(fs::read_to_string(&board).unwrap(), before);
}

#[test]
fn rejected_edit_is_exit_1() {
    let (dir, board) = empty_project();
    let before = fs::read_to_string(&board).unwrap();
    let r = run(dir.path(), &["assign", "periph", "SPI1", "MOSI=PA6"]);
    assert_eq!(r.code, 1);
    insta::assert_snapshot!(r.stdout);
    assert_eq!(fs::read_to_string(&board).unwrap(), before);
}

#[test]
fn bad_signal_spec_is_exit_2() {
    let (dir, _) = empty_project();
    let r = run(dir.path(), &["assign", "periph", "SPI1", "SCK"]);
    assert_eq!(r.code, 2);
    assert!(
        r.stderr.contains("expected SIGNAL=PIN[:NET], got SCK"),
        "{}",
        r.stderr
    );
}

#[test]
fn json_report() {
    let (dir, _) = empty_project();
    let args = ["assign", "periph", "SPI1", "SCK=PA5", "--dry-run", "--json"];
    let r = run(dir.path(), &args);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["written"], false);
    assert_eq!(v["rejected"], false);
    let diff = v["files"][0]["diff"].as_str().unwrap();
    assert!(diff.contains("+    \"SPI1\": {"), "{diff}");
}
