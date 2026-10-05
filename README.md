# pinmap

[![CI](https://github.com/JustBrenkman/pinmap/actions/workflows/ci.yml/badge.svg)](https://github.com/JustBrenkman/pinmap/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

One source of truth for microcontroller pin assignments.

pinmap keeps what a chip's pins *can* do and what your board *chose* to do with them in plain-text files. It checks the assignments against the chip, edits them safely from the command line, and generates a pinout document and a machine-readable pin table from them.

> **Status: early development.** Checking, inspecting, editing, and Markdown/JSON output work today. Devicetree output, schematic verification, and importers for vendor pin data are designed but not built yet; see the [roadmap](#roadmap). The file format may still change before `pinmap/v1` is frozen.

## Why

Pin assignments usually live in a spreadsheet, a vendor tool's project file, the schematic, and a devicetree file, all maintained by hand and all slightly out of sync. pinmap replaces that with:

- **Plain-text files** in JSON or [RON](https://github.com/ron-rs/ron) that diff cleanly in git and are easy for scripts and AI tools to read and write.
- **Checks against the chip:** a signal on a pin that can't carry it, a pin used twice, a pin with no pad in your package, and more, each with a stable error code and a hint.
- **Safe edits:** every edit is checked before it is written, and an edit that would add an error is refused.
- **Generated output** that can't drift from the source: a Markdown pinout today, devicetree later.

## The two documents

| Document | Holds | Files |
|---|---|---|
| **pindef** | What the chip can do: pins, their functions and mux values, and package pads | `chip.pindef.ron`, `chip.pindef.json` |
| **pinasg** | What your board chose: signals on pins, net labels, electrical settings, notes | `board.pinasg.ron`, `board.pinasg.json` |

The file name decides the kind and format: `<name>.<kind>.<format>`. JSON and RON hold exactly the same data and convert to each other without loss.

A pindef lists each pin's functions. Until importers exist, you write it by hand for the pins you use:

```ron
(
    schema: "pinmap/v1",
    kind: pindef,
    device: (name: "STM32F407VGTx", vendor: "st"),
    packages: {
        "LQFP100": {"PA4": "29", "PA5": "30"},
    },
    pins: {
        "PA4": (functions: [
            (periph: "SPI1", name: "NSS", mux: af(5)),
            (periph: "DAC", name: "OUT1", mux: analog),
        ]),
        "PA5": (functions: [
            (periph: "SPI1", name: "SCK", mux: af(5)),
            (periph: "TIM2", name: "CH1", mux: af(1)),
        ]),
    },
)
```

A pinasg refers to its pindef and says what you chose:

```ron
#![enable(implicit_some)]
(
    schema: "pinmap/v1",
    kind: pinasg,
    device: "../devices/stm32f407vg",
    target: (package: "LQFP100", refdes: "U1"),
    peripherals: {
        "SPI1": (
            signals: {
                "SCK": (pin: "PA5", net: "IMU_SCK", speed: very_high),
            },
            props: {"spi-max-frequency": "<8000000>"},
            note: "IMU bus",
        ),
    },
    pins: {
        "PA4": (mode: gpio_out, initial: high, net: "IMU_CS", note: "SPI1 CS, active low"),
    },
)
```

The alternate-function number for `SPI1.SCK` on `PA5` is never written in the pinasg. It is looked up in the pindef, so the two can't disagree. `device` is a path without the `.pindef.<format>` ending, so a file can change format without breaking references. A pinasg can split into several files with `imports: ["usb", "debug"]`.

A complete example project is in [`fixtures/json`](fixtures/json) (and the same project as RON in [`fixtures/ron`](fixtures/ron)). Its pin numbers and AF values are illustrative; check them against the datasheet before relying on them.

## Install

pinmap needs a recent stable Rust toolchain.

```sh
cargo install --git https://github.com/JustBrenkman/pinmap pinmap-cli
```

Try it on the example project:

```sh
git clone https://github.com/JustBrenkman/pinmap.git
cd pinmap/fixtures/json
pinmap check
pinmap pin list
```

## Usage

There is no project file to set up. pinmap looks under the current directory for the root pinasg, the one that no other file imports. If a tree holds several boards, choose one with `--project board/revb.pinasg.ron`.

Every command takes `--json` for scripts and AI tools. Exit codes are 0 when the command ran clean, 1 when it found problems, and 2 when it could not run.

### Check and inspect

```sh
pinmap check                     # validate the assignments against the pindef
pinmap pin list                  # every pin with its pad, use, and net
pinmap pin list --free           # pins with a pad that nothing uses
pinmap pin list --can SPI1.SCK   # pins that can carry a signal, with the mux
pinmap pin show PA5              # one pin: functions, use, settings, where it is set
pinmap periph list
pinmap periph show SPI1          # signals, chosen pins, candidates, props
```

`check` reports problems like this:

```
error[PM0102] SPI1.MOSI: PA6 has no function SPI1_MOSI (functions: SPI1_MISO, TIM3_CH1)
              board/revb.pinasg.json  peripherals.SPI1.signals.MOSI
              hint: pinmap pin list --can SPI1.MOSI
✘ 1 error
```

The error codes are listed in [docs/design.md](docs/design.md#checks).

### Edit

```sh
pinmap assign periph SPI1 SCK=PA5:IMU_SCK MISO=PA6:IMU_MISO --speed very-high
pinmap assign pin PA4 --mode gpio-out --initial high --net IMU_CS
pinmap prop set SPI1 spi-max-frequency '<8000000>'
pinmap note PA4 "SPI1 CS, active low"
pinmap unassign PA4                              # or a peripheral, or SPI1.SCK
pinmap assign periph SPI1 MOSI=PA6 --dry-run     # show the diff and any new errors
```

- An edit that would add an error is refused: nothing is written and the command exits 1. `--allow-errors` writes anyway. Errors that were already there don't block unrelated edits.
- Changes land in whichever file defines the entry, even an imported one. New entries go to the root file, or to `--file`.
- Files are rewritten in canonical form, atomically. If an edit touches several files, either all of them are written or none.
- A RON file with comments is not rewritten, since that would drop the comments, unless you pass `--allow-comment-loss`.

### Generate output

```sh
pinmap emit --to markdown -o docs/pinout.md   # a pin table plus a section per peripheral
pinmap emit --to json > pinout.json           # a flat [{pin, pad, function, mux, net}] table
pinmap emit --to markdown -O free-pins=true   # also list unused pins
pinmap list emitters                          # every format and its options
```

`emit` refuses a project with errors (exit 1), so a broken assignment never ends up in your docs; `--allow-errors` overrides. Output is the same whether your files are JSON, RON, or a mix.

### Files and formats

```sh
pinmap fmt                       # rewrite the project's files in canonical form
pinmap fmt --check               # in CI: exit 1 if any file isn't canonical
pinmap convert board.pinasg.json board.pinasg.ron --rm-source
pinmap schema pinasg > pinasg.schema.json
```

`fmt --check` skips RON files with comments rather than failing them. `pinmap schema pindef|pinasg` prints a JSON Schema, which editors can use for autocomplete and validation of `*.pinasg.json` and `*.pindef.json` files.

## Using the libraries

The crates can be used without the CLI. To read and write documents:

```rust
use pinmap_model::{read, write, PinAsg};

let asg: PinAsg = read("board/revb.pinasg.ron")?;
write("board/revb.pinasg.json", &asg)?;
```

To load, merge, and check a whole project:

```rust
let (resolved, diagnostics) = pinmap_core::open("board/revb.pinasg.ron".as_ref());
```

| Crate | What it holds | State |
|---|---|---|
| `pinmap-model` | Document types, file names and formats, canonical reading and writing | Done |
| `pinmap-core` | Loading and merging projects, checks, the edit pipeline, the `Emitter` trait | Done; `verify` to come |
| `pinmap-emit` | Emitters | Markdown and JSON |
| `pinmap-cli` | The `pinmap` binary | Done for the features above |
| `pinmap-dts` | Devicetree AST for the Zephyr and Linux emitters | Not started |
| `pinmap-import` | Importers for vendor pin data | Not started |

The design behind all of this, including the data model and the rules for checks and edits, is in [docs/design.md](docs/design.md).

## Roadmap

| | Scope | State |
|---|---|---|
| Core | Model, loader with imports, checks, inspection, editing | Done |
| Output | Emitter interface, Markdown and JSON | Done |
| Verify | Read a KiCad netlist and report net labels that don't match the assignments | Planned |
| Devicetree | Devicetree AST and a Zephyr emitter, then Linux | Planned |
| Importers | Generate pindefs from vendor data (STM32 first), added as projects need them | Planned |
| Second vendor | nRF, then freeze the `pinmap/v1` schema | Planned |
| Breadth | Mermaid diagrams, adapters for other tools | Planned |

## Contributing

Contributions are welcome. [CONTRIBUTING.md](CONTRIBUTING.md) covers building, testing, and adding a check rule or an emitter.

## License

[MIT](LICENSE)
