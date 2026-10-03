mod common;

use pinmap_core::{Severity, open};

use common::fixtures;

/// (code, file name, logical path) for every diagnostic of a fixture.
fn findings(root: &str) -> Vec<(&'static str, String, String)> {
    let (_, diags) = open(&fixtures().join(root));
    diags
        .iter()
        .map(|d| {
            let file = d.file.as_ref().unwrap().file_name().unwrap();
            let path = d.path.clone().unwrap_or_default();
            (d.code, file.to_string_lossy().into_owned(), path)
        })
        .collect()
}

fn one(code: &'static str, file: &str, path: &str) -> Vec<(&'static str, String, String)> {
    vec![(code, file.to_owned(), path.to_owned())]
}

#[test]
fn clean_fixtures_have_no_findings() {
    for root in [
        "json/board/revb.pinasg.json",
        "ron/board/revb.pinasg.ron",
        "mixed/board/revb.pinasg.ron",
        "check/diamond/root.pinasg.json",
    ] {
        let found = findings(root);
        assert!(found.is_empty(), "{root}: {found:?}");
    }
}

#[test]
fn each_rule_fixture_gives_exactly_its_finding() {
    let board = "board.pinasg.json";
    let cases = [
        (
            "PM0101",
            one("PM0101", board, "peripherals.SPI1.signals.SCK"),
        ),
        (
            "PM0102",
            one("PM0102", board, "peripherals.SPI1.signals.MOSI"),
        ),
        ("PM0103", one("PM0103", board, "pins.PA5")),
        ("PM0104", one("PM0104", board, "pins.P2")),
        ("PM0105", one("PM0105", board, "target.package")),
        ("PM0106", one("PM0106", board, "peripherals.SPI9")),
        ("PM0107", one("PM0107", "loop.pinasg.json", "imports[0]")),
        ("PM0110", one("PM0110", board, "pins.PB12")),
    ];
    for (code, expected) in cases {
        let root = format!("check/{code}/board.pinasg.json");
        assert_eq!(findings(&root), expected, "{root}");
    }
}

#[test]
fn messages_name_the_problem() {
    let (_, diags) = open(&fixtures().join("check/PM0102/board.pinasg.json"));
    assert_eq!(
        diags[0].message,
        "SPI1.MOSI: PA6 has no function SPI1_MOSI (functions: SPI1_MISO, TIM3_CH1)"
    );
    assert_eq!(
        diags[0].hint.as_deref(),
        Some("pinmap pin list --can SPI1.MOSI")
    );

    let (_, diags) = open(&fixtures().join("check/PM0103/board.pinasg.json"));
    assert_eq!(diags[0].message, "PA5 is already used by SPI1.SCK");

    let (_, diags) = open(&fixtures().join("check/PM0110/board.pinasg.json"));
    assert_eq!(diags[0].severity, Severity::Info);
    assert_eq!(diags[0].message, "net LED is on both PA4 and PB12");
}
