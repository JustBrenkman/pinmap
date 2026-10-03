# Changelog

All notable changes to this project are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `pinmap-model`: the `pindef` and `pinasg` document types, file name classification, canonical JSON and RON reading and writing, and reference resolution by path or stem.
- Fixture project stored in both formats, with cross-format round-trip tests.
- `pinmap-core`: project loader with imports, cycle and mismatch detection, the merged `Resolved` view, check rules PM0101–PM0110, and root discovery.
- `pinmap` commands: `check`, `pin list`, `pin show`, `periph list`, `periph show`, `convert`, `fmt`, `schema`, all with `--json`.
- `pinmap-model`: optional `schema` feature, `AnyDoc`, and RON comment detection.
- Edit commands: `assign periph`, `assign pin`, `unassign`, `prop set|unset`, `note`, with `--dry-run` diffs, rejection of edits that add errors (`--allow-errors`), a RON comment guard (`--allow-comment-loss`), and `--file` for where new entries go.
- Atomic writes for every command that changes files.
- `pinmap emit --to markdown|json` with `-o`, `--out-dir`, `-O free-pins=true`, and `--allow-errors`; `pinmap list emitters`; the `Emitter` interface in `pinmap-core`.
- "Did you mean" suggestions for mistyped pins, peripherals, and `--can` signals.
- Workspace layout with stub crates for the loader, devicetree AST, importers, emitters, and CLI.
