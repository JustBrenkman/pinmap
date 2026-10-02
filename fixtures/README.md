# Fixtures

A small STM32F407 project used by the tests.

- `json/` and `ron/` hold the same project, once in each format. They live in separate trees because a stem that matches both a `.json` and a `.ron` file is ambiguous by design.
- `mixed/` is the RON board files next to the JSON pindef, to test a RON pinasg referencing a JSON pindef by stem.

All files are in canonical form: reading one and writing it back gives the same bytes. The tests in `crates/pinmap-model/tests/fixtures.rs` enforce this, so regenerate both formats when you change one.

Pin numbers and alternate-function values are illustrative, not checked against the datasheet.
