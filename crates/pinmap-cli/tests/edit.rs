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

#[test]
fn pin_prop_note_unassign() {
    let (dir, board) = empty_project();
    let d = dir.path();
    let pin = [
        "assign",
        "pin",
        "PA4",
        "--mode",
        "gpio-out",
        "--initial",
        "high",
        "--net",
        "CS",
    ];
    assert_eq!(run(d, &pin).code, 0);
    assert_eq!(run(d, &["assign", "periph", "SPI1", "SCK=PA5"]).code, 0);
    let prop = ["prop", "set", "SPI1", "spi-max-frequency", "<8000000>"];
    assert_eq!(run(d, &prop).code, 0);
    assert_eq!(run(d, &["note", "PA4", "chip select"]).code, 0);
    let text = fs::read_to_string(&board).unwrap();
    assert!(text.contains("\"note\": \"chip select\""), "{text}");
    assert!(
        text.contains("\"spi-max-frequency\": \"<8000000>\""),
        "{text}"
    );

    assert_eq!(run(d, &["note", "PA4", "--clear"]).code, 0);
    let unset = ["prop", "unset", "SPI1", "spi-max-frequency"];
    assert_eq!(run(d, &unset).code, 0);
    assert_eq!(run(d, &["unassign", "PA4"]).code, 0);
    assert_eq!(run(d, &["unassign", "SPI1.SCK"]).code, 0);
    let after = fs::read_to_string(&board).unwrap();
    assert!(
        !after.contains("peripherals") && !after.contains("pins"),
        "{after}"
    );
}

#[test]
fn edit_errors_are_exit_2() {
    let (dir, _) = empty_project();
    let r = run(dir.path(), &["assign", "pin", "PA4"]);
    assert_eq!(r.code, 2);
    assert!(
        r.stderr.contains("give it a mode with --mode"),
        "{}",
        r.stderr
    );
    let r = run(dir.path(), &["unassign", "PA4"]);
    assert_eq!(r.code, 2);
    assert!(r.stderr.contains("PA4 is not assigned"), "{}", r.stderr);
    let r = run(dir.path(), &["note", "PA4"]);
    assert_eq!(r.code, 2, "note needs TEXT or --clear");
}

#[test]
fn build_a_project_from_the_cli() {
    let (dir, board) = empty_project();
    let d = dir.path();
    let steps: &[&[&str]] = &[
        &[
            "assign",
            "periph",
            "SPI1",
            "SCK=PA5:IMU_SCK",
            "MISO=PA6:IMU_MISO",
            "MOSI=PA7:IMU_MOSI",
            "--speed",
            "very-high",
        ],
        &[
            "assign",
            "periph",
            "USART2",
            "TX=PA2:UART_TX_DEBUG",
            "RX=PA3:UART_RX_DEBUG",
        ],
        &["assign", "periph", "USART2", "RX=PA3", "--bias", "pull-up"],
        &[
            "assign",
            "pin",
            "PA4",
            "--mode",
            "gpio-out",
            "--initial",
            "high",
            "--net",
            "IMU_CS",
        ],
        &[
            "assign", "pin", "PB12", "--mode", "gpio-in", "--bias", "pull-up", "--net", "BTN_USER",
        ],
        &[
            "assign", "pin", "PA13", "--mode", "reserved", "--net", "SWDIO",
        ],
        &["prop", "set", "SPI1", "spi-max-frequency", "<8000000>"],
        &["prop", "set", "USART2", "current-speed", "<115200>"],
        &["note", "SPI1", "IMU bus; CS is a GPIO on PA4"],
        &["note", "PA4", "SPI1 CS, active low"],
        &["note", "PA13", "SWD"],
    ];
    for step in steps {
        let r = run(d, step);
        assert_eq!(r.code, 0, "{step:?}: {}{}", r.stdout, r.stderr);
    }
    // A rejected edit leaves the file alone.
    let before = fs::read_to_string(&board).unwrap();
    assert_eq!(run(d, &["assign", "periph", "SPI1", "MOSI=PA6"]).code, 1);
    assert_eq!(fs::read_to_string(&board).unwrap(), before);

    assert_eq!(run(d, &["check"]).stdout, "✔ no problems\n");
    assert_eq!(run(d, &["fmt", "--check"]).code, 0);
    insta::assert_snapshot!(fs::read_to_string(&board).unwrap());
}
