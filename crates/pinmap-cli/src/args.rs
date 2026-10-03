use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use pinmap_model::{DocKind, Format};

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
    /// Inspect pins.
    #[command(subcommand)]
    Pin(PinCommand),
    /// Inspect peripherals.
    #[command(subcommand)]
    Periph(PeriphCommand),
    /// Convert a document between JSON and RON.
    Convert(ConvertArgs),
    /// Rewrite documents in canonical form.
    Fmt(FmtArgs),
    /// Print the JSON Schema for a document kind.
    Schema { kind: KindArg },
}

#[derive(Args)]
pub struct CheckArgs {
    /// Fail on warnings too.
    #[arg(long)]
    pub deny_warnings: bool,
}

#[derive(Subcommand)]
pub enum PinCommand {
    /// List the device's pins with their use and net.
    List(PinListArgs),
    /// Show everything about one pin.
    Show {
        /// Pin name, for example PA5.
        pin: String,
    },
}

#[derive(Args)]
pub struct PinListArgs {
    /// Only pins with a pad that nothing uses.
    #[arg(long, conflicts_with = "used")]
    pub free: bool,
    /// Only pins something uses.
    #[arg(long)]
    pub used: bool,
    /// Only pins that can carry this signal, for example SPI1.SCK.
    #[arg(long, value_name = "PERIPH.SIGNAL")]
    pub can: Option<String>,
    /// Only pins with this net label.
    #[arg(long)]
    pub net: Option<String>,
}

#[derive(Subcommand)]
pub enum PeriphCommand {
    /// List the device's peripherals and which are assigned.
    List,
    /// Show a peripheral's signals, candidate pins, and assignment.
    Show {
        /// Peripheral name, for example SPI1.
        periph: String,
    },
}

#[derive(Args)]
pub struct ConvertArgs {
    /// Input file, or `-` for stdin.
    pub input: PathBuf,
    /// Output file. Omit when reading stdin; the result goes to stdout.
    pub output: Option<PathBuf>,
    /// Delete the input after a successful conversion.
    #[arg(long)]
    pub rm_source: bool,
    /// Overwrite the output if it exists.
    #[arg(long)]
    pub force: bool,
    /// Document kind, when reading stdin.
    #[arg(long)]
    pub kind: Option<KindArg>,
    /// Input format, when reading stdin.
    #[arg(long)]
    pub from: Option<FormatArg>,
    /// Output format, when reading stdin.
    #[arg(long)]
    pub to: Option<FormatArg>,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum KindArg {
    Pindef,
    Pinasg,
}

impl From<KindArg> for DocKind {
    fn from(kind: KindArg) -> Self {
        match kind {
            KindArg::Pindef => DocKind::Pindef,
            KindArg::Pinasg => DocKind::Pinasg,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
pub enum FormatArg {
    Json,
    Ron,
}

impl From<FormatArg> for Format {
    fn from(format: FormatArg) -> Self {
        match format {
            FormatArg::Json => Format::Json,
            FormatArg::Ron => Format::Ron,
        }
    }
}

#[derive(Args)]
pub struct FmtArgs {
    /// Report files that would change instead of rewriting them.
    #[arg(long)]
    pub check: bool,
    /// Files to format. Defaults to the project's files and its pindef.
    pub paths: Vec<PathBuf>,
}
