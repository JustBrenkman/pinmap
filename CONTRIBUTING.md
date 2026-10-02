# Contributing to pinmap

Thanks for your interest. pinmap is early: the document model exists and the rest is designed but not built. Read [docs/design.md](docs/design.md) first, since it explains what each crate is for and what was deliberately left out.

## Building and testing

You need a recent stable Rust toolchain.

```sh
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```

`pinmap-model` puts each file format behind a cargo feature, so also run:

```sh
cargo test -p pinmap-model --no-default-features --features json
cargo test -p pinmap-model --no-default-features --features ron
```

CI runs all of the above.

## Crate boundaries

The dependency rule is enforced by the crate graph. Please keep it that way.

| Crate | May depend on | Must not depend on |
|---|---|---|
| `pinmap-model` | serde, indexmap | everything else in the workspace |
| `pinmap-import` | model | emitters, other importers |
| `pinmap-core` | model | importers, emitters |
| `pinmap-emit` | model, core | importers, other emitters |
| `pinmap-cli` | all | |

## Changing the document model

- Use plain serde derives: no `untagged`, no `flatten`, no custom visitors.
- Put `deny_unknown_fields` on every struct, so a typo in a file is an error.
- Use `IndexMap` for maps, so files keep their order and output is deterministic.
- Schema field names and enum variants are `snake_case`.
- New fields should be optional and skipped when absent, so existing files stay valid.
- Update the fixtures in both `fixtures/json` and `fixtures/ron`. They must stay in canonical form and the tests check this.

## Adding an importer or emitter

These are the main extension points. Neither trait exists yet; once `pinmap-core` defines them, the steps will be:

1. Add a module in `pinmap-import` or `pinmap-emit` and implement the trait.
2. Add one `register()` line in `pinmap-cli`, behind a cargo feature.
3. Add a fixture and a golden output file.

Importer quality matters more than emitter breadth: an incomplete function list makes every later check wrong.

## Commits and pull requests

- Keep commit messages to a single line in the imperative mood, for example `Add nRF importer`.
- Keep pull requests focused on one change, with tests.
- For anything that changes the file formats or adds back something the design cut, open an issue first.

## License

By contributing, you agree that your contributions are licensed under the [MIT license](LICENSE).
