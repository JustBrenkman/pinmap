//! The `pinmap` binary.
//!
//! No subcommands are implemented yet. See `docs/design.md` section 8.

use clap::Parser;

/// One source of truth for microcontroller pin assignments.
#[derive(Parser)]
#[command(name = "pinmap", version, about)]
struct Cli {}

fn main() {
    Cli::parse();
}
