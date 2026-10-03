use std::path::{Path, PathBuf};

use pinmap_core::{Usage, open};
use pinmap_model::{Mode, Mux};

fn fixtures() -> PathBuf {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    crates.parent().unwrap().join("fixtures")
}

#[test]
fn merges_root_and_imports() {
    let (resolved, diags) = open(&fixtures().join("json/board/revb.pinasg.json"));
    assert!(diags.is_empty(), "{diags:?}");
    let r = resolved.unwrap();

    let names: Vec<&str> = r.peripherals.keys().map(String::as_str).collect();
    assert_eq!(names, ["SPI1", "USART2", "OTG_FS"]);
    let usb = &r.peripherals["OTG_FS"];
    assert!(usb.origin.file.ends_with("board/usb.pinasg.json"));
    assert_eq!(usb.origin.path, "peripherals.OTG_FS");
    assert_eq!(
        usb.signals["DM"].origin.path,
        "peripherals.OTG_FS.signals.DM"
    );
}

#[test]
fn derives_mux_and_indexes_usage() {
    let (resolved, _) = open(&fixtures().join("json/board/revb.pinasg.json"));
    let r = resolved.unwrap();

    assert_eq!(r.peripherals["SPI1"].signals["SCK"].mux, Some(Mux::Af(5)));
    assert_eq!(
        r.uses("PA5"),
        [Usage::Signal {
            periph: "SPI1".into(),
            signal: "SCK".into()
        }]
    );
    assert_eq!(
        r.uses("PA4"),
        [Usage::Pin {
            mode: Mode::GpioOut
        }]
    );
    assert!(r.uses("PA0").is_empty());
    assert_eq!(r.pad("PA5"), Some("30"));
    assert_eq!(r.net("PA4"), Some("IMU_CS"));
    assert_eq!(r.net("PA5"), Some("IMU_SCK"));
    assert_eq!(r.uses("PA4")[0].to_string(), "gpio_out");
    assert_eq!(r.uses("PA5")[0].to_string(), "SPI1.SCK");

    let spi_sck: Vec<&str> = r
        .pins_with_function("SPI1", "SCK")
        .map(|(id, _)| id.as_str())
        .collect();
    assert_eq!(spi_sck, ["PA5"]);
    assert!(r.device_peripherals().contains(&"OTG_FS"));
}

#[test]
fn key_defined_twice_keeps_the_first() {
    let (resolved, diags) = open(&fixtures().join("check/PM0108/board.pinasg.json"));
    let found: Vec<(&str, Option<&str>)> =
        diags.iter().map(|d| (d.code, d.path.as_deref())).collect();
    assert_eq!(
        found,
        [
            ("PM0108", Some("peripherals.USART2")),
            ("PM0108", Some("pins.PB12"))
        ]
    );
    assert!(
        diags
            .iter()
            .all(|d| d.file.as_ref().unwrap().ends_with("dup.pinasg.json"))
    );
    assert!(
        diags[0].message.contains("board.pinasg.json"),
        "{}",
        diags[0].message
    );

    let r = resolved.unwrap();
    assert!(r.peripherals["USART2"].signals.contains_key("TX"));
    assert_eq!(r.pins["PB12"].asg.mode, Mode::GpioIn);
}

#[test]
fn no_resolved_view_without_a_pindef() {
    let (resolved, diags) = open(Path::new("nowhere/board.pinasg.json"));
    assert!(resolved.is_none());
    assert_eq!(diags[0].code, "PM0008");
}

#[test]
fn settings_describe_each_use() {
    let (resolved, _) = open(&fixtures().join("json/board/revb.pinasg.json"));
    let r = resolved.unwrap();
    let signal = |periph: &str, signal: &str| Usage::Signal {
        periph: periph.into(),
        signal: signal.into(),
    };
    let settings = |pin: &str, usage: &Usage| r.assignment(pin, usage).settings();
    assert_eq!(settings("PA5", &signal("SPI1", "SCK")), ["speed=very_high"]);
    assert_eq!(settings("PA3", &signal("USART2", "RX")), ["bias=pull_up"]);
    let pin = Usage::Pin {
        mode: Mode::GpioOut,
    };
    assert_eq!(settings("PA4", &pin), ["initial=high"]);
}
