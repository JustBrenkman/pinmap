mod common;

use common::{fixtures, pinmap};

#[test]
fn prints_the_pinasg_schema() {
    let run = pinmap(&fixtures(), &["schema", "pinasg"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let schema: serde_json::Value = serde_json::from_str(&run.stdout).unwrap();
    assert_eq!(schema["title"], "PinAsg");
    assert!(schema["properties"]["peripherals"].is_object());
    assert_eq!(schema["additionalProperties"], false);
}

#[test]
fn rejects_unknown_kinds() {
    let run = pinmap(&fixtures(), &["schema", "board"]);
    assert_eq!(run.code, 2);
}
