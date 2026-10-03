# pinmap: Design

A Rust CLI that stores pin definitions and pin assignments as structured, text-editable, serde-based documents, and generates devicetree (Zephyr/Linux), documentation, and diagrams from them. It also checks that a KiCad schematic's net labels match the assignments.

Status: design v3 (pared back). Pin numbers, AF values, and Zephyr label names in the examples are from memory and illustrative. Verify them against datasheets and your Zephyr version before relying on them.

---

## 1. Goals and non-goals

### Goals

- One source of truth for pin definitions and assignments, replacing spreadsheets and XML.
- Plain-text files that open in any editor, diff cleanly in git, and are easy for scripts and AI tools to read and write.
- A strict **import -> IR -> export** pipeline. New importers and emitters are the main extension point.
- Usable without opening files: list, assign, check, and verify from the CLI, with `--json` output everywhere.
- Let an AI (or a human) check that the schematic's net labels match the intended assignments.

### Non-goals (deliberately cut)

| Cut | Reason / replacement |
|---|---|
| Decisions log | Git history. A free-text `note` field covers one-line rationale. |
| Constraints, intent, locks | `--dry-run` and git are the safety net. |
| Board document, components, bindings | The schematic is the source for components. |
| `observed` document | The KiCad netlist is parsed in memory by `verify` and never stored. |
| Nets as entities | A `net` label string on a pin or signal. |
| Presets, `status`, signal shorthand | Added complexity for little gain. Presence in the file means enabled. |
| DTS child nodes (`imu@0` etc.) | Highly board-specific. Hand tweaking of generated DTS is expected. |
| Plugin system | Possibly a WASM runtime later. For now, everything lives in this repo. |
| Op enum, batch apply, solver, MCP server | Later. `--json` output already makes the CLI scriptable. |

Everything cut can return as an optional field plus a schema bump without breaking existing files.

---

## 2. Document types and file naming

Two document kinds, each available in JSON or RON.

| Document | Purpose | Typical author | JSON | RON |
|---|---|---|---|---|
| **pindef** (pin definitions) | What the chip can do: pins, functions, mux, package pads | Importer | `x.pindef.json` | `x.pindef.ron` |
| **pinasg** (pin assignments) | What we chose: signal-to-pin mapping, net labels, config | Human / CLI / AI | `x.pinasg.json` | `x.pinasg.ron` |

Rules:

- Filename pattern: `<name>.<kind>.<format>`. The stem may contain dots.
- The `kind` field inside the file must match the extension (`PM0002` on mismatch).
- `schema: "pinmap/v1"` in every file. No `$schema` key (not a valid RON identifier). Bind editors to the JSON Schema by filename glob.
- Supported formats: JSON and RON, each behind a cargo feature. YAML is out for now (anchor/alias problems, boolean pitfalls, crate churn). Adding it back is one enum variant.
- All schema field names and enum variants are `snake_case` (RON identifiers can't contain `-`). Data keys (devicetree property names like `"clock-frequency"`) keep their natural spelling.

### Fact ownership

| Fact | Owner |
|---|---|
| Pin functions, mux values (AF number etc.), package pad numbers | pindef |
| Which pin carries which signal, electrical config, net label, notes | pinasg |
| AF number for an assigned signal | **Derived** by lookup, never stored in pinasg |
| Pin-to-signal inverse view | **Derived** |

### Reference resolution

`device:` and `imports:` accept an explicit path or a stem:

- Explicit extension: use exactly that file.
- Stem (recommended): the field implies the kind (`device` is pindef, `imports` entries are pinasg). The loader probes `.ron` then `.json`.
- If both exist, it is a hard error (`PM0004`). No silent preference.
- Stems mean converting a file's format never requires editing the files that reference it.

### Format behavior

- Edits never change a file's format. Use `pinmap convert` to change it.
- New files take `default_format` from `pinmap.toml`, but an explicit extension wins.
- Suggested convention: JSON for importer-generated pindefs; RON for hand-edited pinasgs.
- Mixed formats in one project work (a RON pinasg can reference a JSON pindef).
- Output is canonical (stable key order, fixed indent, trailing newline), so `a -> b -> a` conversion is byte-stable.
- RON files are written with `#![enable(implicit_some)]` so optional fields don't need `Some(...)`.
- Use `floating` for "no bias", not `none` (RON can read `None` as an absent option).

---

## 3. Data model

### 3.1 pindef (importer output)

```json
{
  "schema": "pinmap/v1",
  "kind": "pindef",
  "device": { "name": "STM32F407VGTx", "vendor": "st", "family": "stm32f4" },
  "packages": {
    "LQFP100": { "PA2": "25", "PA4": "29", "PA5": "30", "PA6": "31", "PA7": "32" }
  },
  "pins": {
    "PA5": {
      "port": { "bank": "A", "index": 5 },
      "functions": [
        { "periph": "SPI1", "name": "SCK",  "mux": { "af": 5 } },
        { "periph": "TIM2", "name": "CH1",  "mux": { "af": 1 } },
        { "periph": "DAC",  "name": "OUT2", "mux": "analog" }
      ]
    }
  }
}
```

- `packages` maps logical pin to physical pad (strings, since BGAs use `A1`, `K10`). This is the bridge to the schematic.
- `periph` and `name` are split by the importer, which has the vendor knowledge (for example `SYS_JTMS-SWDIO` becomes `SYS` + `JTMS-SWDIO`). The core never parses signal strings.
- The `Mux` enum is `Af(u8) | FuncSel(u8) | Any | Analog | Fixed | Vendor(map)`, so the IR does not assume STM32 (RP2040 function select, nRF any-pin routing, i.MX mux/pad triples all fit).
- Generated pindef files are read-only to edit commands. Change them by re-running the importer (`pinmap refresh`).
- Peripheral base addresses and IRQs (from SVD) are not included in v1. They can be added later as an optional `peripherals` section.

### 3.2 pinasg (hand-written or CLI-edited)

```ron
#![enable(implicit_some)]
(
    schema: "pinmap/v1",
    kind: pinasg,
    device: "../devices/stm32f407vg",
    target: (package: "LQFP100", refdes: "U1"),
    imports: ["usb"],

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
        "USART2": (
            signals: {
                "TX": (pin: "PA2", net: "UART_TX_DEBUG"),
                "RX": (pin: "PA3", net: "UART_RX_DEBUG", bias: pull_up),
            },
            props: { "current-speed": "<115200>" },
        ),
    },

    pins: {
        "PA4":  (mode: gpio_out, initial: high, net: "IMU_CS", note: "SPI1 CS, active low"),
        "PB12": (mode: gpio_in, bias: pull_up, net: "BTN_USER"),
        "PA13": (mode: reserved, net: "SWDIO", note: "SWD"),
    },
)
```

Design points:

- A signal is a map entry under its peripheral, so one signal on two pins cannot be expressed. The remaining check is one pin used by two signals.
- `net` is a plain label string: exactly what the schematic check needs.
- `props` values are raw devicetree text (`"<8000000>"`): no value type to design, exact match for the DTS output. Non-DTS emitters ignore `props`.
- `mode: reserved` marks pins with no GPIO role (SWD etc.) so they don't show as free.
- `refdes` is the MCU's reference designator in the schematic (used by `verify`).
- `imports` merge by union. A key defined in two files is a hard error (no override rules).
- Presence of a peripheral means it is enabled (no `status` field).

### 3.3 Rust types (sketch)

```rust
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct PinAsg {
    pub schema: SchemaVersion,
    pub kind: DocKind,
    pub device: PathBuf,
    pub target: Target,                          // package + optional refdes
    #[serde(default)] pub imports: Vec<PathBuf>,
    #[serde(default)] pub peripherals: IndexMap<String, PeriphAsg>,
    #[serde(default)] pub pins: IndexMap<PinId, PinAsgEntry>,
}

pub struct PeriphAsg {
    pub signals: IndexMap<String, SignalAsg>,
    #[serde(default)] pub props: IndexMap<String, String>,
    pub note: Option<String>,
}
pub struct SignalAsg {
    pub pin: PinId,
    pub net: Option<String>,
    pub bias: Option<Bias>, pub drive: Option<Drive>, pub speed: Option<Speed>,
}
pub struct PinAsgEntry {
    pub mode: Mode,                              // gpio_in | gpio_out | analog | reserved
    pub initial: Option<Level>, pub bias: Option<Bias>,
    pub net: Option<String>, pub note: Option<String>,
}

pub enum DocKind { Pindef, Pinasg }              // serialized lowercase
pub enum Format  { Json, Ron }
```

Serde rules:

- Plain derive only: no `untagged`, no `flatten`, no custom visitors.
- `deny_unknown_fields` on every struct, so typos like `pul: up` are errors.
- `IndexMap` (serde feature) everywhere, to keep file order and make output deterministic.
- Externally tagged enums: `af(5)` in RON, `{"af": 5}` in JSON.

---

## 4. Architecture

```
vendor data ──► IMPORTERS ──► *.pindef.* ─┐
                                          ├─► load + merge ─► RESOLVED ─► EMITTERS ─► files
              *.pinasg.* (+ imports) ─────┘                      │
                                                                 ├─► check
 kicad-cli netlist (in memory only) ─────────────────────────────┴─► verify
```

Two IR levels:

- **Raw**: exactly what is in a file, with imports unresolved. Round-trips through serde.
- **Resolved**: a validated graph with both lookup directions (pin -> signal and signal -> pin), derived mux values, and provenance (file plus logical path). Emitters consume only this.

### Workspace layout

```
crates/
  pinmap-model/    serde types, Format, classify(), read/write. No other logic.
  pinmap-core/     loader/imports, resolve, check, verify, edit pipeline, Importer/Emitter traits
  pinmap-dts/      small devicetree AST + printer (shared by Zephyr/Linux)
  pinmap-import/   one module per importer, cargo-feature gated: stm32, kicad_lib, svd
  pinmap-emit/     one module per emitter, cargo-feature gated: zephyr, linux, markdown, mermaid, json
  pinmap-cli/      clap, registry wiring
fixtures/          real chips and boards, with golden outputs
```

### Dependency rule (enforced by crate boundaries)

| Layer | May depend on | Must not depend on |
|---|---|---|
| model | serde, indexmap | everything else |
| importers | model | emitters, other importers |
| core | model | importers, emitters |
| emitters | model, core (read-only `Resolved`) | importers, other emitters |
| cli | all | n/a |

### Traits

```rust
pub trait Importer {
    fn id(&self) -> &'static str;
    fn import(&self, src: &Path, opts: &Options) -> Result<PinDef, ImportError>;
}
pub trait Emitter {
    fn id(&self) -> &'static str;
    fn emit(&self, ir: &Resolved, opts: &Options) -> Result<EmitOutput, EmitError>;
}
pub struct EmitOutput { pub files: Vec<OutputFile>, pub unsupported: Vec<Diagnostic> }
```

- Emitters return files in memory. This makes snapshot testing trivial. The CLI decides whether to write them.
- `unsupported` reports anything the backend could not express. It never drops data silently.

### Adding an importer or emitter

1. Add a module and implement the trait.
2. Add one `register()` line in `pinmap-cli`, behind a cargo feature.
3. Add a fixture and a golden file.

---

## 5. Loading, imports, and validation (`check`)

### Loader

- Paths resolve relative to the importing file.
- Cycles are detected with a visited stack. Diamond imports load once (keyed by canonical path).
- Pre-scan for comments before any rewrite (see section 7).
- Diagnostics carry file plus logical path (`peripherals.SPI1.signals.MOSI`). serde does not give line numbers. Keep provenance as a type so spans can be added later.

### Checks

| Code | Check | Severity |
|---|---|---|
| PM0101 | Pin not in the pindef | error |
| PM0102 | Pin has no function matching `(periph, signal)` | error |
| PM0103 | Pin used by two signals, or by both a signal and a `pins` entry | error |
| PM0104 | Pin has no pad for `target.package` | error |
| PM0105 | `target.package` not in the pindef | error |
| PM0106 | Peripheral has no functions in the pindef | error |
| PM0107 | Import cycle | error |
| PM0108 | Same peripheral or pin key defined in two files | error |
| PM0109 | Import's device or target differs from the root's | error |
| PM0110 | Same `net` label on two different pins | info |
| PM0001–PM0008 | File name, kind/extension mismatch, schema version, ambiguous or missing reference, parse and I/O errors | error |

Every `pinasg`, including imported ones, declares `device` and `target`, and an import must match the root's. A "net but no function or mode" warning was planned but dropped: every `pins` entry requires a `mode`, and every signal is already checked against the pindef, so the v1 types cannot express it.

Diagnostics have stable codes (`PM0102` etc.), name the file and path, and suggest a follow-up command:

```
error[PM0102] SPI1.MOSI: PA6 has no function SPI1_MOSI (functions: SPI1_MISO, TIM3_CH1, ...)
              board/revb.pinasg.ron  peripherals.SPI1.signals.MOSI
              hint: pinmap pin list --can SPI1.MOSI
```

---

## 6. Schematic verification (`verify`)

Goal: confirm the MCU's signal pins carry the net labels the assignments say they should. Power pins and passives are out of scope.

### Input

- Run `kicad-cli sch export netlist` (KiCad 7+; path configurable), or accept a pre-exported netlist file for CI.
- Parse the XML netlist (`quick-xml`) in memory into a map of `refdes.pad -> net name`. Prefer the netlist over parsing `.kicad_sch`: it is already flattened across hierarchical sheets and power symbols.
- Check which pin-name attribute (`pinfunction`) your KiCad version emits, since the `pin_name_mismatch` check depends on it.

### Algorithm

1. For each assigned pin with a `net`, look up its pad from `packages[target.package]`, then find `refdes.pad` in the netlist.
2. Normalize the schematic net name by stripping the sheet path (`/IMU_SCK` and `/sensors/IMU_SCK` both become `IMU_SCK`).
3. Compare with the expected `net`.

### Findings

| Code | Finding | Meaning |
|---|---|---|
| PM0412 | `unconnected` | Pad has no net |
| PM0413 | `wrong_label` | Pad is on a different named net (with swap detection for pairs) |
| PM0414 | `unlabeled` | Pad is on an auto-named net (`Net-(U2-SCLK)`); the label is missing |
| PM0415 | `unexpected` | Pad has a labeled net but no assignment (warn, with a configurable ignore list) |
| PM0416 | `pin_name_mismatch` | KiCad pin name disagrees with the pindef (wrong symbol or package) |

```
$ pinmap verify
error[PM0412] PA4 (U1.29) is unconnected; expected IMU_CS
error[PM0413] PA6 (U1.31) is on "IMU_MOSI"; expected IMU_MISO  (SPI1.MISO)
error[PM0413] PA7 (U1.32) is on "IMU_MISO"; expected IMU_MOSI  (SPI1.MOSI)
              labels look swapped
error[PM0414] PA5 (U1.30) is on auto-named net "Net-(U2-SCLK)"; add label IMU_SCK
✘ 4 errors
```

`--json` gives the same findings as structured data (code, severity, pin, pad, expected, found, origin file and path). The tool reports and never decides which side is wrong. An AI can fix the schematic labels or change the assignment with `pinmap assign`.

---

## 7. Edit pipeline

Edit commands call functions directly (no Op layer yet):

```
load project -> apply change -> re-resolve whole project -> run check
  -> reject if it introduces new errors -> write atomically (temp file + rename)
```

- Pre-existing errors do not block an unrelated edit. `--allow-errors` overrides.
- `--dry-run` prints a diff and the diagnostic changes and writes nothing.
- Generated files (`meta.generated: true` pindefs) are refused with "re-run import".
- New entries go to the root pinasg, or `--file`. Edits to existing entries go to the file that defines them.
- **Comments:** JSON has none, so editing it never loses any. For RON, the writer scans for comments first and refuses unless `--allow-comment-loss` is given. Use JSON for files that the CLI or an AI edits heavily and RON for files you hand-edit and annotate. `note` fields are real data and survive in both.

---

## 8. CLI

```
# project and import
pinmap init [dir]                                 # phase 2, with pinmap.toml
pinmap import --from stm32 <src> --mcu STM32F407VGTx -o devices/stm32f407vg.pindef.json
pinmap import --from kicad-lib sym.kicad_sym -o devices/
pinmap refresh                                    # re-run importers listed in pinmap.toml

# inspect (all support --json)
pinmap pin list [--free|--used] [--can SPI1.SCK] [--net IMU_SCK]
pinmap pin show PA5
pinmap periph list | show SPI1

# edit (all support --dry-run)
pinmap assign periph SPI1 SCK=PA5:IMU_SCK MISO=PA6:IMU_MISO   # SIGNAL=PIN[:NET]
pinmap assign pin PA4 --mode gpio-out --initial high --net IMU_CS
pinmap unassign PA5 | SPI1 | SPI1.SCK
pinmap prop set SPI1 spi-max-frequency '<8000000>'
pinmap prop unset SPI1 spi-max-frequency
pinmap note PA4 "SPI1 CS, active low"
pinmap note PA4 --clear

# validate and output
pinmap check [--deny-warnings]
pinmap verify [--netlist FILE]
pinmap emit --to zephyr|linux|markdown|mermaid|json [--out-dir D] [-O key=value]
pinmap build                                      # every [emit.*] target in pinmap.toml

# format and tooling
pinmap convert in.pinasg.json out.pinasg.ron [--rm-source]
pinmap convert - --kind pinasg --from json --to ron
pinmap fmt [paths...]
pinmap schema pindef | pinasg
pinmap list importers|emitters
```

Implementation notes:

- `--from` and `--to` are validated against the runtime registry (`PossibleValuesParser`), so help text and shell completions (`clap_complete`) stay accurate.
- Every command returns a `Serialize + JsonSchema` struct. The CLI renders it as a table or as JSON, so output is never scraped from text.
- `pinmap schema` uses `schemars`. Editors can bind the schema by glob (`*.pinasg.json`) for autocomplete and validation.

### Finding the project

There is no required project file. The root pinasg is `--project FILE`, or else the single pinasg under the current directory that no other pinasg imports. Hidden directories and `target/` are skipped. When several roots exist (several boards in one tree), the command stops and lists them.

### Manifest (phase 2, optional)

`pinmap.toml` arrives with the importers, because `[sources]` and `[emit]` have nowhere else to live. Everything in phase 1 works without it.

```toml
# pinmap.toml (optional; discovered by walking up from cwd)
[project]
primary        = "board/revb.pinasg.ron"
default_format = "ron"
schematic      = "hw/revb.kicad_sch"

[sources.mcu]
importer = "stm32"
mcu      = "STM32F407VGTx"
out      = "devices/stm32f407vg.pindef.json"

[emit.zephyr]
out = "build/zephyr"

[emit.markdown]
out = "docs/pinout.md"
```

`[sources]` makes generated pindefs reproducible: `pinmap refresh` re-runs importers and the git diff shows what the vendor data changed.

---

## 9. Importers

| Importer | Produces | Source / crate | Notes |
|---|---|---|---|
| `stm32` | pindef | ST `STM32_open_pin_data` (XML via `quick-xml`) or embassy `stm32-data` JSON | Pins, AFs, packages. Embassy's JSON is a possible shortcut. |
| `kicad_lib` | pindef | `.kicad_sym` S-expressions | Package pad <-> pin name table. Needed for `verify`. Second source for parts with no vendor database. |
| `svd` | extends pindef | `svd-parser` | Later. Adds optional peripheral base addresses and IRQs. |
| `xlsx` | pindef/pinasg | `calamine` | Later. Migration only. |

Important: **SVD contains no pin data** (no packages, balls, or alternate-function tables). It only supplies peripheral registers and IRQs. Check each vendor's data license before redistributing generated files.

---

## 10. Emitters

### Zephyr devicetree

- Built from an AST in `pinmap-dts` (nodes, labels, properties, phandle references, `/delete-property/`), not string concatenation.
- Output is a starting overlay that you tweak, not a finished board file.

```dts
/* generated by pinmap: tweak as needed */
&spi1 {
	status = "okay";
	pinctrl-0 = <&spi1_sck_pa5 &spi1_miso_pa6 &spi1_mosi_pa7>;
	pinctrl-names = "default";
	spi-max-frequency = <8000000>;
};
```

- AF numbers are derived from the pindef function table, never from the pinasg.
- `bias`, `drive`, `speed` map to pinctrl properties where Zephyr has them. Anything without a mapping goes to `unsupported`.
- Each vendor family needs its own dialect module (STM32 `STM32_PINMUX`, nRF `NRF_PSEL` groups, i.MX mux/pad register triples). This is the biggest source of ongoing work.
- Verify label names and what the HAL `.dtsi` already sets against your Zephyr version. Run emitted output through `dtc` in CI.

### Linux devicetree

Same AST, different dialect (`pinctrl-0` plus the family's pinmux macros, such as `STM32_PINMUX('A', 5, AF5)` for STM32MP). Added after Zephyr proves the AST.

### Documentation and diagrams

- **Markdown:** pin table (pin, pad, function, mux, net, config, note). This is the spreadsheet replacement.
- **Mermaid:** peripheral -> pin -> net-label edges, one diagram per peripheral (whole-chip diagrams exceed practical limits).
- **JSON:** flat `[{pin, pad, function, mux, net}]` table for any script. This is also the starting point for Wokwi or pinout-editor adapters, each of which is a transform of that table. The pinout-editor input format is not confirmed yet, so treat it as an adapter slot.

### Later

KiCad spec (wiring list as JSON), `.kicad_sym` generation, C and Rust header emitters, a `minijinja` template emitter (user templates against the resolved IR as data).

---

## 11. Testing

- **Golden/snapshot tests** (`insta`) per emitter, using fixtures from real boards.
- **Cross-format fixtures:** each fixture is stored in JSON and RON.
  1. Both load to equal values.
  2. JSON -> RON -> JSON and RON -> JSON -> RON are byte-identical.
  3. `classify` table tests (`a.b.pinasg.ron` works, `x.ron` and `x.pinasg.yaml` do not).
  4. Ambiguous-stem and kind-mismatch errors.
  5. A RON pinasg referencing a JSON pindef by stem.
  6. Enum cases: `mux: af(5)` versus `{"af": 5}`, and `floating` bias.
- **Rule tests:** each check gets a minimal failing and passing fixture.
- **CLI tests** with `assert_cmd`.
- **`dtc` validation** of emitted DTS; optionally a CI job that builds an emitted Zephyr overlay against a real board.
- **Importer fuzzing** for the XML and S-expression parsers.
- **Verify fixtures** built from messy real schematics (hierarchical sheets, auto-named nets, bus labels), not only clean ones.
- **Emitter conformance harness** (generic over the registry): output is deterministic, independent of map ordering, never panics across the fixture corpus.

---

## 12. Roadmap

| Phase | Scope | Exit criterion |
|---|---|---|
| **1. Core** | Model, `Format`, `classify`, loader with imports and stem resolution, resolver, `check`, `pin`/`periph` inspection, edit pipeline, canonical writer, root discovery | A hand-written project is edited entirely through the CLI, with clean diffs and rollback on error (met) |
| **2. STM32 slice** | Optional `pinmap.toml`, `init`, `refresh`, STM32 importer, `pinmap-dts` AST, Zephyr emitter, Markdown emitter, conformance harness | A Nucleo project's overlay passes `dtc` and builds |
| **3. Verify** | KiCad netlist reader, `kicad_lib` importer, `verify` | A real schematic verifies clean; a deliberately broken one gives the expected findings |
| **4. Second vendor** | nRF (any-pin routing), then freeze `pinmap/v1` | The schema needed no STM32-specific changes |
| **5. Breadth** | Linux emitter, Mermaid, JSON adapters, SVD, `build` | n/a |

Do phase 4 before freezing the schema: only a second, structurally different vendor will reveal STM32 assumptions in the model.

---

## 13. Risks

- **Vendor dialect growth.** DTS pinmux encoding differs per vendor and OS. This will be the largest area of work. The `Mux` enum and per-dialect modules contain it.
- **Importer completeness.** If a pindef's `functions` list is incomplete, `check` and any AI using `pin list --can` will give wrong answers confidently. Importer quality matters more than emitter breadth.
- **Net-label matching in `verify`.** Hierarchical sheets, global versus local labels, and bus labels are the likely source of false positives. Budget real time and real fixtures.
- **Comment loss on rewrite.** Mitigated by the comment guard, `note` fields, and the JSON/RON per-file choice.
- **YAML crate/format instability** is avoided by dropping YAML.
- **External tool dependency.** `verify` shells out to `kicad-cli` unless given a pre-exported netlist. Make the path configurable and keep the netlist-file path first-class.

---

## 14. Deferred features (additive, when needed)

- Per-peripheral `cs: [...]` for `cs-gpios`.
- A `role` on GPIO pins (`button`, `led`) mapped to `gpio-keys` and `gpio-leds`.
- Typed `props` values in place of raw DTS strings.
- Presets for repeated electrical config.
- Optional SVD-derived `peripherals` section in pindef (base addresses, IRQs) for full node generation.
- MCP server wrapping the existing `--json` commands.
- A `suggest` command (pin candidates for a peripheral, honoring free pins).
- KiCad spec and symbol emitters; direct `.kicad_sch` generation (hardest, last).
- WASM-based importer/emitter extensions.
- Spanned diagnostics (line and column) via a span-aware parser.
- Board/components document and a net model, if schematic checking later needs more than labels.

---

## 15. Open questions

1. Which pinout-editor input format is the target? The adapter is a transform of the JSON table once the format is known.
2. Which Zephyr and Linux versions and SoC families come first beyond STM32 (nRF? i.MX? STM32MP)?
3. Should `verify` ignore power pins by default, or accept a configurable ignore list of pads and net-name patterns?
4. Is `refdes` in `target` enough for multi-chip boards, or will an IO expander or second MCU need multiple targets in one project?

---

## 16. Immediate next step

Write `pinmap-model`: `DocKind`, `Format`, `classify`, the pindef/pinasg types, reference resolution, and the cross-format round-trip tests against two small fixtures. This settles the serde and RON questions (`implicit_some`, externally tagged enums) before the schema freezes.
