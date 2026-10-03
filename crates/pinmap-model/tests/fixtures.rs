//! Cross-format tests against the fixture project, which is stored once as
//! JSON and once as RON.

#![cfg(all(feature = "json", feature = "ron"))]

use std::fs;
use std::path::{Path, PathBuf};

use pinmap_model::{
    AnyDoc, DocKind, Document, Format, PinAsg, PinDef, classify, from_str, read, read_any,
    resolve_ref, to_string,
};

const PINDEFS: [&str; 1] = ["devices/stm32f407vg.pindef"];
const PINASGS: [&str; 2] = ["board/revb.pinasg", "board/usb.pinasg"];

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn text(tree: &str, name: &str, format: Format) -> String {
    let path = fixtures().join(tree).join(format!("{name}.{format}"));
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("{}: {err}", path.display()))
}

/// Checks one document stored in both formats.
fn check_pair<T: Document + PartialEq + std::fmt::Debug>(name: &str) {
    let json_text = text("json", name, Format::Json);
    let ron_text = text("ron", name, Format::Ron);

    let from_json: T = from_str(&json_text, Format::Json).unwrap();
    let from_ron: T = from_str(&ron_text, Format::Ron).unwrap();
    assert_eq!(from_json, from_ron, "{name}: formats disagree");

    // Fixtures are stored in canonical form.
    assert_eq!(
        to_string(&from_json, Format::Json).unwrap(),
        json_text,
        "{name}"
    );
    assert_eq!(
        to_string(&from_ron, Format::Ron).unwrap(),
        ron_text,
        "{name}"
    );

    // JSON -> RON -> JSON is byte-identical.
    let as_ron = to_string(&from_json, Format::Ron).unwrap();
    assert_eq!(as_ron, ron_text, "{name}: JSON -> RON");
    let back: T = from_str(&as_ron, Format::Ron).unwrap();
    assert_eq!(
        to_string(&back, Format::Json).unwrap(),
        json_text,
        "{name}: JSON -> RON -> JSON"
    );

    // RON -> JSON -> RON is byte-identical.
    let as_json = to_string(&from_ron, Format::Json).unwrap();
    assert_eq!(as_json, json_text, "{name}: RON -> JSON");
    let back: T = from_str(&as_json, Format::Json).unwrap();
    assert_eq!(
        to_string(&back, Format::Ron).unwrap(),
        ron_text,
        "{name}: RON -> JSON -> RON"
    );
}

#[test]
fn pindefs_round_trip_across_formats() {
    for name in PINDEFS {
        check_pair::<PinDef>(name);
    }
}

#[test]
fn pinasgs_round_trip_across_formats() {
    for name in PINASGS {
        check_pair::<PinAsg>(name);
    }
}

#[test]
fn map_order_follows_the_file() {
    let def: PinDef = read(fixtures().join("json/devices/stm32f407vg.pindef.json")).unwrap();
    let pads: Vec<&str> = def.packages["LQFP100"]
        .keys()
        .map(|pin| pin.as_str())
        .collect();
    // PB12 sits between PA7 and PA11 in the file, which no sorted order gives.
    assert_eq!(&pads[5..8], ["PA7", "PB12", "PA11"]);
}

#[test]
fn ron_pinasg_references_json_pindef_by_stem() {
    let board = fixtures().join("mixed/board");
    let asg: PinAsg = read(board.join("revb.pinasg.ron")).unwrap();

    let device = resolve_ref(&board, &asg.device, DocKind::Pindef).unwrap();
    assert_eq!(classify(&device).unwrap(), (DocKind::Pindef, Format::Json));
    let def: PinDef = read(&device).unwrap();
    assert_eq!(def.device.name, "STM32F407VGTx");

    let import = resolve_ref(&board, &asg.imports[0], DocKind::Pinasg).unwrap();
    assert_eq!(classify(&import).unwrap(), (DocKind::Pinasg, Format::Ron));
    let usb: PinAsg = read(&import).unwrap();
    assert!(usb.peripherals.contains_key("OTG_FS"));
}

#[test]
fn stem_is_ambiguous_when_both_formats_sit_together() {
    let dir = tempfile::tempdir().unwrap();
    for (tree, format) in [("json", Format::Json), ("ron", Format::Ron)] {
        let name = format!("stm32f407vg.pindef.{format}");
        fs::copy(
            fixtures().join(tree).join("devices").join(&name),
            dir.path().join(&name),
        )
        .unwrap();
    }
    let err = resolve_ref(dir.path(), Path::new("stm32f407vg"), DocKind::Pindef).unwrap_err();
    assert_eq!(err.code(), "PM0004");
}

#[test]
fn every_assigned_pin_has_a_matching_function_in_the_pindef() {
    // The fixture should be a valid project, so later crates can rely on it.
    let root = fixtures().join("json");
    let def: PinDef = read(root.join("devices/stm32f407vg.pindef.json")).unwrap();
    for name in PINASGS {
        let asg: PinAsg = read(root.join(format!("{name}.json"))).unwrap();
        let pads = &def.packages[&asg.target.package];
        for (periph, periph_asg) in &asg.peripherals {
            for (signal, signal_asg) in &periph_asg.signals {
                let pin = &def.pins[signal_asg.pin.as_str()];
                assert!(
                    pin.functions
                        .iter()
                        .any(|f| &f.periph == periph && &f.name == signal),
                    "{name}: {} has no function {periph}.{signal}",
                    signal_asg.pin
                );
                assert!(pads.contains_key(signal_asg.pin.as_str()));
            }
        }
        for pin in asg.pins.keys() {
            assert!(
                def.pins.contains_key(pin.as_str()),
                "{name}: unknown pin {pin}"
            );
            assert!(pads.contains_key(pin.as_str()));
        }
    }
}

#[test]
fn read_any_dispatches_on_the_file_name() {
    let def = read_any(fixtures().join("json/devices/stm32f407vg.pindef.json")).unwrap();
    assert_eq!(def.kind(), DocKind::Pindef);

    let asg = read_any(fixtures().join("ron/board/revb.pinasg.ron")).unwrap();
    assert_eq!(asg.kind(), DocKind::Pinasg);
    assert_eq!(
        asg.to_string(Format::Ron).unwrap(),
        text("ron", "board/revb.pinasg", Format::Ron)
    );
    assert_eq!(
        asg.to_string(Format::Json).unwrap(),
        text("json", "board/revb.pinasg", Format::Json)
    );
}

#[test]
fn any_doc_from_str_checks_the_kind() {
    let json = text("json", "board/usb.pinasg", Format::Json);
    let doc = AnyDoc::from_str(&json, DocKind::Pinasg, Format::Json).unwrap();
    assert!(matches!(doc, AnyDoc::Pinasg(_)));
    let err = AnyDoc::from_str(&json, DocKind::Pindef, Format::Json).unwrap_err();
    assert_eq!(err.code(), "PM0002");
}
