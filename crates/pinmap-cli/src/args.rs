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
    /// Inspect pins.
    #[command(subcommand)]
    Pin(PinCommand),
    /// Inspect peripherals.
    #[command(subcommand)]
    Periph(PeriphCommand),
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
