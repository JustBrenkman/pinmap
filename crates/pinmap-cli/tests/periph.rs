mod common;

use common::{fixtures, pinmap};

#[test]
fn periph_list() {
    let run = pinmap(&fixtures().join("json"), &["periph", "list"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    insta::assert_snapshot!(run.stdout);
}

#[test]
fn periph_show_assigned() {
    let run = pinmap(&fixtures().join("json"), &["periph", "show", "SPI1"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    insta::assert_snapshot!(run.stdout);
}

#[test]
fn periph_show_unassigned() {
    let run = pinmap(&fixtures().join("json"), &["periph", "show", "TIM3"]);
    assert_eq!(run.code, 0);
    assert!(
        run.stdout.starts_with("TIM3  not assigned\n"),
        "{}",
        run.stdout
    );
    assert!(
        run.stdout.contains("CH1     -    -    -    PA6"),
        "{}",
        run.stdout
    );
}

#[test]
fn periph_show_json() {
    let args = ["periph", "show", "SPI1", "--json"];
    let run = pinmap(&fixtures().join("json"), &args);
    let value: serde_json::Value = serde_json::from_str(&run.stdout).unwrap();
    assert_eq!(value["signals"][0]["signal"], "SCK");
    assert_eq!(value["signals"][0]["mux"], serde_json::json!({"af": 5}));
    assert_eq!(value["props"]["spi-max-frequency"], "<8000000>");
}

#[test]
fn periph_show_suggests_case() {
    let run = pinmap(&fixtures().join("json"), &["periph", "show", "spi1"]);
    assert_eq!(run.code, 2);
    assert!(run.stderr.contains("did you mean SPI1?"), "{}", run.stderr);
}

#[test]
fn periph_show_suggests_near_typos() {
    let run = pinmap(&fixtures().join("json"), &["periph", "show", "SPl1"]);
    assert_eq!(run.code, 2);
    assert!(run.stderr.contains("did you mean SPI1?"), "{}", run.stderr);
}

#[test]
fn periph_show_includes_signal_config() {
    let run = pinmap(&fixtures().join("json"), &["periph", "show", "SPI1"]);
    assert!(run.stdout.contains("speed=very_high"), "{}", run.stdout);
}
