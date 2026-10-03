use std::fs;
use std::path::{Path, PathBuf};

use pinmap_core::{EditError, EditOptions, EditOutcome, Session, SignalConfig, SignalSpec};
use pinmap_model::Speed;

fn fixtures() -> PathBuf {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    crates.parent().unwrap().join("fixtures")
}

/// A project with the fixture pindef and a board file holding `body`
/// (JSON members after `target`, may be empty).
fn project(body: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("devices")).unwrap();
    fs::create_dir_all(dir.path().join("board")).unwrap();
    fs::copy(
        fixtures().join("json/devices/stm32f407vg.pindef.json"),
        dir.path().join("devices/stm32f407vg.pindef.json"),
    )
    .unwrap();
    let root = dir.path().join("board/board.pinasg.json");
    let sep = if body.is_empty() { "" } else { ", " };
    fs::write(
        &root,
        format!(
            r#"{{"schema": "pinmap/v1", "kind": "pinasg", "device": "../devices/stm32f407vg",
                "target": {{"package": "LQFP100", "refdes": "U1"}}{sep}{body}}}"#
        ),
    )
    .unwrap();
    (dir, root)
}

/// A copy of the fixture project, which has an imported file.
fn revb() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    for sub in ["devices", "board"] {
        fs::create_dir_all(dir.path().join(sub)).unwrap();
        for entry in fs::read_dir(fixtures().join("json").join(sub)).unwrap() {
            let path = entry.unwrap().path();
            fs::copy(&path, dir.path().join(sub).join(path.file_name().unwrap())).unwrap();
        }
    }
    let root = dir.path().join("board/revb.pinasg.json");
    (dir, root)
}

fn spec(signal: &str, pin: &str, net: Option<&str>) -> SignalSpec {
    SignalSpec {
        signal: signal.into(),
        pin: pin.into(),
        net: net.map(str::to_owned),
    }
}

fn write() -> EditOptions {
    EditOptions::default()
}

fn assign(root: &Path, periph: &str, specs: &[SignalSpec], options: &EditOptions) -> EditOutcome {
    let mut session = Session::open(root).unwrap();
    session
        .assign_signals(periph, specs, &SignalConfig::default())
        .unwrap();
    session.commit(options).unwrap()
}

#[test]
fn assigns_a_new_peripheral() {
    let (_dir, root) = project("");
    let mut session = Session::open(&root).unwrap();
    let config = SignalConfig {
        speed: Some(Speed::VeryHigh),
        ..Default::default()
    };
    session
        .assign_signals("SPI1", &[spec("SCK", "PA5", Some("IMU_SCK"))], &config)
        .unwrap();
    let outcome = session.commit(&write()).unwrap();
    assert!(outcome.written && !outcome.rejected);
    assert!(outcome.added.is_empty(), "{:?}", outcome.added);
    assert_eq!(outcome.files.len(), 1);
    let text = fs::read_to_string(&root).unwrap();
    assert_eq!(text, outcome.files[0].after);
    assert!(text.contains("\"speed\": \"very_high\""), "{text}");
}

#[test]
fn changing_a_pin_keeps_the_net() {
    let (_dir, root) = project("");
    assign(&root, "TIM2", &[spec("CH1", "PA5", Some("PWM"))], &write());
    let mut session = Session::open(&root).unwrap();
    let config = SignalConfig {
        speed: Some(Speed::Low),
        ..Default::default()
    };
    session
        .assign_signals("TIM2", &[spec("CH1", "PA5", None)], &config)
        .unwrap();
    session.commit(&write()).unwrap();
    let text = fs::read_to_string(&root).unwrap();
    assert!(text.contains("\"net\": \"PWM\""), "{text}");
    assert!(text.contains("\"speed\": \"low\""), "{text}");
}

#[test]
fn edits_that_add_errors_are_rejected() {
    let (_dir, root) = project("");
    let before = fs::read_to_string(&root).unwrap();
    let outcome = assign(&root, "SPI1", &[spec("MOSI", "PA6", None)], &write());
    assert!(outcome.rejected && !outcome.written);
    assert_eq!(outcome.added[0].code, "PM0102");
    assert_eq!(outcome.files.len(), 1, "the diff is still reported");
    assert_eq!(fs::read_to_string(&root).unwrap(), before);
}

#[test]
fn allow_errors_writes_anyway() {
    let (_dir, root) = project("");
    let options = EditOptions {
        allow_errors: true,
        ..Default::default()
    };
    let outcome = assign(&root, "SPI1", &[spec("MOSI", "PA6", None)], &options);
    assert!(!outcome.rejected && outcome.written);
}

#[test]
fn existing_errors_do_not_block() {
    let (_dir, root) = project(r#""pins": {"PZ9": {"mode": "gpio_out"}}"#);
    let outcome = assign(&root, "SPI1", &[spec("SCK", "PA5", None)], &write());
    assert!(outcome.written && outcome.added.is_empty() && outcome.resolved.is_empty());
}

#[test]
fn fixing_one_error_does_not_excuse_a_new_one() {
    let (_dir, root) = project(r#""peripherals": {"SPI1": {"signals": {"SCK": {"pin": "PA6"}}}}"#);
    // PA6 has no SPI1_SCK. Moving SCK to PA5 fixes that; MOSI on PA6 adds a new error.
    let specs = [spec("SCK", "PA5", None), spec("MOSI", "PA6", None)];
    let outcome = assign(&root, "SPI1", &specs, &write());
    assert!(outcome.rejected);
    assert_eq!(outcome.resolved.len(), 1);
    assert_eq!(outcome.added.len(), 1);
}

#[test]
fn dry_run_writes_nothing() {
    let (_dir, root) = project("");
    let before = fs::read_to_string(&root).unwrap();
    let options = EditOptions {
        dry_run: true,
        ..Default::default()
    };
    let outcome = assign(&root, "SPI1", &[spec("SCK", "PA5", None)], &options);
    assert!(!outcome.written && !outcome.rejected);
    assert_eq!(outcome.files[0].before, before);
    assert_eq!(fs::read_to_string(&root).unwrap(), before);
}

#[test]
fn repeating_an_edit_changes_nothing() {
    let (_dir, root) = project("");
    assign(&root, "SPI1", &[spec("SCK", "PA5", Some("X"))], &write());
    let outcome = assign(&root, "SPI1", &[spec("SCK", "PA5", Some("X"))], &write());
    assert!(outcome.files.is_empty() && !outcome.written);
}

#[test]
fn edits_land_in_the_defining_file() {
    let (dir, root) = revb();
    let outcome = assign(
        &root,
        "OTG_FS",
        &[spec("DM", "PA11", Some("USB_DM"))],
        &write(),
    );
    assert!(outcome.written, "{:?}", outcome.added);
    assert_eq!(outcome.files.len(), 1);
    assert!(outcome.files[0].path.ends_with("board/usb.pinasg.json"));
    let usb = fs::read_to_string(dir.path().join("board/usb.pinasg.json")).unwrap();
    assert!(usb.contains("USB_DM"));
}

#[test]
fn comment_guard_blocks_ron() {
    let (dir, _) = project("");
    let ron = dir.path().join("board/board.pinasg.ron");
    let json = dir.path().join("board/board.pinasg.json");
    let doc: pinmap_model::PinAsg = pinmap_model::read(&json).unwrap();
    let text = pinmap_model::to_string(&doc, pinmap_model::Format::Ron).unwrap();
    fs::write(&ron, format!("// hand notes\n{text}")).unwrap();
    fs::remove_file(&json).unwrap();

    let mut session = Session::open(&ron).unwrap();
    session
        .assign_signals(
            "SPI1",
            &[spec("SCK", "PA5", None)],
            &SignalConfig::default(),
        )
        .unwrap();
    let options = EditOptions {
        dry_run: true,
        ..Default::default()
    };
    let err = session.commit(&options).unwrap_err();
    assert!(matches!(err, EditError::CommentLoss { .. }), "{err}");

    let mut session = Session::open(&ron).unwrap();
    session
        .assign_signals(
            "SPI1",
            &[spec("SCK", "PA5", None)],
            &SignalConfig::default(),
        )
        .unwrap();
    let options = EditOptions {
        allow_comment_loss: true,
        ..Default::default()
    };
    assert!(session.commit(&options).unwrap().written);
    assert!(!fs::read_to_string(&ron).unwrap().contains("hand notes"));
}

#[test]
fn open_needs_a_pindef() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("board.pinasg.json");
    fs::write(
        &root,
        r#"{"schema": "pinmap/v1", "kind": "pinasg", "device": "nope", "target": {"package": "P"}}"#,
    )
    .unwrap();
    assert!(matches!(Session::open(&root), Err(EditError::Load(_))));
}
