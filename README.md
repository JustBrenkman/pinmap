# pinmap

[![CI](https://github.com/JustBrenkman/pinmap/actions/workflows/ci.yml/badge.svg)](https://github.com/JustBrenkman/pinmap/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

One source of truth for microcontroller pin assignments.

pinmap stores what a chip's pins *can* do and what your board *chose* to do with them as plain-text documents, then generates devicetree, documentation, and diagrams from them. It also checks that a KiCad schematic's net labels match the assignments.

> **Status: early development.** The document model, the project loader and checks, and the read-only CLI work today. Editing commands, importers, emitters, and schematic verification are designed but not built yet. See the [roadmap](#roadmap).

## Why

Pin assignments usually live in a spreadsheet, a vendor GUI's XML, a schematic, and a devicetree file, all maintained by hand and all slightly out of sync. pinmap replaces that with:

- **Plain-text files** that open in any editor, diff cleanly in git, and are easy for scripts and AI tools to read and write.
- **A strict import -> IR -> export pipeline.** Vendor data comes in through importers; devicetree, Markdown, and diagrams come out through emitters.
- **A scriptable CLI** to list, assign, check, and verify without opening files, with `--json` output everywhere.
- **Schematic verification** that reports where the schematic's net labels disagree with the intended assignments.

## The two documents

| Document | Purpose | Typical author | Files |
|---|---|---|---|
| **pindef** | What the chip can do: pins, functions, mux values, package pads | An importer | `x.pindef.json`, `x.pindef.ron` |
| **pinasg** | What you chose: signal-to-pin mapping, net labels, electrical config | You, the CLI, or an AI | `x.pinasg.json`, `x.pinasg.ron` |

Both are available in JSON or [RON](https://github.com/ron-rs/ron), and the two formats convert to each other byte-stably. A pin assignment looks like this:

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
                "SCK":  (pin: "PA5", net: "IMU_SCK",  speed: very_high),
                "MISO": (pin: "PA6", net: "IMU_MISO", speed: very_high),
                "MOSI": (pin: "PA7", net: "IMU_MOSI", speed: very_high),
            },
            props: { "spi-max-frequency": "<8000000>" },
            note: "IMU bus; CS is a GPIO on PA4",
        ),
    },

    pins: {
        "PA4": (mode: gpio_out, initial: high, net: "IMU_CS", note: "SPI1 CS, active low"),
    },
)
```

The alternate-function number for `SPI1.SCK` on `PA5` is never written here. It is looked up from the pindef, so the two can't disagree.

Pin numbers, AF values, and devicetree label names in the examples and fixtures are illustrative. Verify them against datasheets before relying on them.

## How it fits together

```
vendor data ──► IMPORTERS ──► *.pindef.* ─┐
                                          ├─► load + merge ─► RESOLVED ─► EMITTERS ─► files
              *.pinasg.* (+ imports) ─────┘                      │
                                                                 ├─► check
 kicad-cli netlist (in memory only) ─────────────────────────────┴─► verify
```

| Crate | Role | State |
|---|---|---|
| `pinmap-model` | Document types, format detection, canonical read/write, reference resolution | Implemented |
| `pinmap-core` | Loader, resolver, `check`; later `verify`, the edit pipeline, and the `Importer`/`Emitter` traits | Partial |
| `pinmap-dts` | Devicetree AST and printer shared by the Zephyr and Linux emitters | Stub |
| `pinmap-import` | Importers: STM32, KiCad symbol libraries, SVD | Stub |
| `pinmap-emit` | Emitters: Zephyr, Linux, Markdown, Mermaid, JSON | Stub |
| `pinmap-cli` | The `pinmap` binary | Read-only commands |

The full design, including the data model, validation rules, and schematic verification algorithm, is in [docs/design.md](docs/design.md).

## Using the CLI

```sh
cargo install --path crates/pinmap-cli

cd my-board                     # any directory above your *.pinasg.* files
pinmap check                    # validate assignments against the pindef
pinmap pin list --free          # pins with a pad that nothing uses
pinmap pin list --can SPI1.SCK  # pins that can carry a signal
pinmap pin show PA5
pinmap periph show SPI1
pinmap convert board.pinasg.json board.pinasg.ron --rm-source
pinmap fmt --check              # in CI
pinmap schema pinasg > pinasg.schema.json
```

There is no project file to set up. pinmap looks under the current directory for the root assignment file, the one `.pinasg` file that no other file imports. With several boards in one tree, choose one with `--project board/revb.pinasg.ron`.

`check` reports problems like this:

```
error[PM0102] SPI1.MOSI: PA6 has no function SPI1_MOSI (functions: SPI1_MISO, TIM3_CH1)
              board/revb.pinasg.json  peripherals.SPI1.signals.MOSI
              hint: pinmap pin list --can SPI1.MOSI
✘ 1 error
```

Every command takes `--json` for scripts and AI tools. Exit codes: 0 when the command ran clean, 1 when it found problems, 2 when it could not run.

Still to come: `import`, `assign`, `verify`, and `emit`.

## Roadmap

| Phase | Scope | State |
|---|---|---|
| 1. Core | Model, loader with imports, resolver, `check`, inspection, edit pipeline | Read-only CLI done; editing next |
| 2. STM32 slice | STM32 importer, devicetree AST, Zephyr and Markdown emitters | Planned |
| 3. Verify | KiCad netlist reader, KiCad library importer, `verify` | Planned |
| 4. Second vendor | nRF, then freeze the `pinmap/v1` schema | Planned |
| 5. Breadth | Linux emitter, Mermaid, JSON adapters, SVD | Planned |

The schema is not frozen until phase 4. Expect breaking changes to the file formats before then.

## Building

Requires a recent stable Rust toolchain.

```sh
git clone https://github.com/JustBrenkman/pinmap.git
cd pinmap
cargo test --workspace
```

Example documents in both formats are under [fixtures/](fixtures).

## Using the model crate

```rust
use pinmap_model::{read, write, PinAsg};

let asg: PinAsg = read("board/revb.pinasg.ron")?;
write("board/revb.pinasg.json", &asg)?;
```

The file name decides the document kind and format (`<name>.<kind>.<format>`), and the `kind` field inside the file must agree with it.

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for how to build, test, and add an importer or emitter.

## License

[MIT](LICENSE)
