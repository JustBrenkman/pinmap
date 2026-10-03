use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// One source of truth for microcontroller pin assignments.
#[derive(Parser)]
#[command(name = "pinmap", version, about)]
pub struct Cli {
    /// Print JSON instead of text.
    #[arg(long, global = true)]
    pub json: bool,

    /// The root pin assignment file. Found automatically when omitted.
    #[arg(long, global = true, value_name = "FILE")]
    pub project: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Validate the project.
    Check(CheckArgs),
}

#[derive(Args)]
pub struct CheckArgs {
    /// Fail on warnings too.
    #[arg(long)]
    pub deny_warnings: bool,
}
