use std::fs;
use std::path::{Path, PathBuf};

use pinmap_core::{
    EditError, EditOptions, EditOutcome, PinChange, Removed, Session, SignalConfig, SignalSpec,
};
use pinmap_model::{Level, Mode, Speed};

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

#[test]
fn new_pin_needs_a_mode() {
    let (_dir, root) = project("");
    let mut session = Session::open(&root).unwrap();
    let err = session
        .assign_pin(&"PA4".into(), &PinChange::default())
        .unwrap_err();
    assert!(matches!(err, EditError::ModeRequired(_)));
}

#[test]
fn assign_pin_creates_then_updates_only_given_fields() {
    let (_dir, root) = project("");
    let mut session = Session::open(&root).unwrap();
    let change = PinChange {
        mode: Some(Mode::GpioOut),
        net: Some("LED".into()),
        ..Default::default()
    };
    session.assign_pin(&"PA4".into(), &change).unwrap();
    session.commit(&EditOptions::default()).unwrap();

    let mut session = Session::open(&root).unwrap();
    let change = PinChange {
        initial: Some(Level::High),
        ..Default::default()
    };
    session.assign_pin(&"PA4".into(), &change).unwrap();
    session.commit(&EditOptions::default()).unwrap();
    let doc: pinmap_model::PinAsg = pinmap_model::read(&root).unwrap();
    let entry = &doc.pins["PA4"];
    assert_eq!(
        (entry.mode, entry.initial, entry.net.as_deref()),
        (Mode::GpioOut, Some(Level::High), Some("LED"))
    );
}

#[test]
fn new_entries_go_to_the_chosen_file() {
    let (dir, root) = revb();
    let mut session = Session::open(&root).unwrap();
    session
        .set_new_entry_file(&dir.path().join("board/usb.pinasg.json"))
        .unwrap();
    session
        .assign_signals(
            "TIM2",
            &[spec("CH3", "PA2", None)],
            &SignalConfig::default(),
        )
        .unwrap();
    let options = EditOptions {
        dry_run: true,
        ..Default::default()
    };
    let outcome = session.commit(&options).unwrap();
    assert!(outcome.files[0].path.ends_with("board/usb.pinasg.json"));
}

#[test]
fn chosen_file_must_be_in_the_project() {
    let (dir, root) = revb();
    let mut session = Session::open(&root).unwrap();
    let outside = dir.path().join("devices/stm32f407vg.pindef.json");
    assert!(matches!(
        session.set_new_entry_file(&outside),
        Err(EditError::NotInProject(_))
    ));
}

fn unassign(root: &Path, name: &str) -> Result<Removed, EditError> {
    let mut session = Session::open(root).unwrap();
    let removed = session.unassign(name)?;
    session.commit(&EditOptions::default()).unwrap();
    Ok(removed)
}

fn doc(path: &Path) -> pinmap_model::PinAsg {
    pinmap_model::read(path).unwrap()
}

#[test]
fn unassign_signal_pin_and_peripheral() {
    let (dir, root) = revb();
    unassign(&root, "SPI1.SCK").unwrap();
    assert!(!doc(&root).peripherals["SPI1"].signals.contains_key("SCK"));

    assert!(matches!(
        unassign(&root, "PA4").unwrap(),
        Removed::Pin { .. }
    ));
    assert!(!doc(&root).pins.contains_key("PA4"));

    unassign(&root, "PA11").unwrap();
    let usb = doc(&dir.path().join("board/usb.pinasg.json"));
    assert!(!usb.peripherals["OTG_FS"].signals.contains_key("DM"));

    assert_eq!(
        unassign(&root, "USART2").unwrap(),
        Removed::Peripheral("USART2".into())
    );
    assert!(!doc(&root).peripherals.contains_key("USART2"));
}

#[test]
fn empty_peripheral_is_removed() {
    let (_dir, root) = project(r#""peripherals": {"SPI1": {"signals": {"SCK": {"pin": "PA5"}}}}"#);
    unassign(&root, "SPI1.SCK").unwrap();
    assert!(doc(&root).peripherals.is_empty());
}

#[test]
fn peripheral_with_props_survives_losing_signals() {
    let (_dir, root) = revb();
    for signal in ["SCK", "MISO", "MOSI"] {
        unassign(&root, &format!("SPI1.{signal}")).unwrap();
    }
    let spi = &doc(&root).peripherals["SPI1"];
    assert!(spi.signals.is_empty() && !spi.props.is_empty());
}

#[test]
fn unassign_unknown_suggests() {
    let (_dir, root) = revb();
    let err = unassign(&root, "PA44").unwrap_err();
    assert_eq!(err.to_string(), "PA44 is not assigned; did you mean PA4?");
}

#[test]
fn unassign_ambiguous_name() {
    let (_dir, root) = project(
        r#""peripherals": {"PA5": {"signals": {"X": {"pin": "PB12"}}}}, "pins": {"PA5": {"mode": "gpio_out"}}"#,
    );
    assert!(matches!(
        unassign(&root, "PA5"),
        Err(EditError::Ambiguous(_))
    ));
}

#[test]
fn props_on_assigned_peripherals_only() {
    let (_dir, root) = revb();
    let mut session = Session::open(&root).unwrap();
    session
        .set_prop("USART2", "current-speed", "<9600>")
        .unwrap();
    assert!(matches!(
        session.set_prop("SPI2", "x", "<1>"),
        Err(EditError::NotAssigned { .. })
    ));
    assert!(matches!(
        session.unset_prop("SPI1", "nope"),
        Err(EditError::NotAssigned { .. })
    ));
    session.unset_prop("SPI1", "spi-max-frequency").unwrap();
    session.commit(&EditOptions::default()).unwrap();
    let d = doc(&root);
    assert_eq!(d.peripherals["USART2"].props["current-speed"], "<9600>");
    assert!(d.peripherals["SPI1"].props.is_empty());
}

#[test]
fn notes_on_pins_and_peripherals() {
    let (_dir, root) = revb();
    let mut session = Session::open(&root).unwrap();
    session.set_note("PA4", Some("chip select")).unwrap();
    session.set_note("SPI1", None).unwrap();
    assert!(matches!(
        session.set_note("PA5", Some("x")),
        Err(EditError::NoteTarget(_))
    ));
    assert!(matches!(
        session.set_note("PC0", Some("x")),
        Err(EditError::NotAssigned { .. })
    ));
    session.commit(&EditOptions::default()).unwrap();
    let d = doc(&root);
    assert_eq!(d.pins["PA4"].note.as_deref(), Some("chip select"));
    assert_eq!(d.peripherals["SPI1"].note, None);
}

/// A board importing `sub/dup`, with `body_root` and `body_dup` as members
/// after `target` (and after `imports` for the root).
fn two_files(body_root: &str, body_dup: &str) -> (tempfile::TempDir, PathBuf, PathBuf) {
    let (dir, root) = project(&format!(r#""imports": ["sub/dup"], {body_root}"#));
    fs::create_dir_all(dir.path().join("board/sub")).unwrap();
    let dup = dir.path().join("board/sub/dup.pinasg.json");
    fs::write(
        &dup,
        format!(
            r#"{{"schema": "pinmap/v1", "kind": "pinasg", "device": "../../devices/stm32f407vg",
                "target": {{"package": "LQFP100", "refdes": "U1"}}, {body_dup}}}"#
        ),
    )
    .unwrap();
    (dir, root, dup)
}

#[cfg(unix)]
#[test]
fn failed_multi_file_write_changes_nothing() {
    use std::os::unix::fs::PermissionsExt;
    let pins = r#""pins": {"PB12": {"mode": "gpio_in"}}"#;
    let (dir, root, _) = two_files(pins, pins);
    let sub = dir.path().join("board/sub");
    let before = fs::read_to_string(&root).unwrap();
    fs::set_permissions(&sub, fs::Permissions::from_mode(0o555)).unwrap();
    if fs::write(sub.join("probe"), "").is_ok() {
        fs::set_permissions(&sub, fs::Permissions::from_mode(0o755)).unwrap();
        return; // running as root: permissions are not enforced
    }
    let mut session = Session::open(&root).unwrap();
    session.unassign("PB12").unwrap();
    let result = session.commit(&EditOptions::default());
    fs::set_permissions(&sub, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(result.is_err());
    assert_eq!(fs::read_to_string(&root).unwrap(), before);
    let leftovers: Vec<_> = fs::read_dir(dir.path().join("board"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains("pinmap-tmp"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

#[test]
fn unassigning_a_duplicated_peripheral_removes_every_definition() {
    let usart = |sig: &str, pin: &str| {
        format!(r#""peripherals": {{"USART2": {{"signals": {{"{sig}": {{"pin": "{pin}"}}}}}}}}"#)
    };
    let (_dir, root, dup) = two_files(&usart("TX", "PA2"), &usart("RX", "PA3"));
    unassign(&root, "USART2").unwrap();
    assert!(!doc(&root).peripherals.contains_key("USART2"));
    assert!(!doc(&dup).peripherals.contains_key("USART2"));

    let (_dir, root, dup) = two_files(&usart("TX", "PA2"), &usart("RX", "PA3"));
    unassign(&root, "USART2.TX").unwrap();
    assert!(!doc(&dup).peripherals.contains_key("USART2"));
}
