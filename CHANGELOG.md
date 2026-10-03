# Changelog

All notable changes to this project are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `pinmap-model`: the `pindef` and `pinasg` document types, file name classification, canonical JSON and RON reading and writing, and reference resolution by path or stem.
- Fixture project stored in both formats, with cross-format round-trip tests.
- `pinmap-core`: project loader with imports, cycle and mismatch detection, the merged `Resolved` view, check rules PM0101–PM0110, and root discovery.
- `pinmap` commands: `check`, `pin list`, `pin show`, `periph list`, `periph show`, `convert`, `fmt`, `schema`, all with `--json`.
- `pinmap-model`: optional `schema` feature, `AnyDoc`, and RON comment detection.
- Workspace layout with stub crates for the loader, devicetree AST, importers, emitters, and CLI.
