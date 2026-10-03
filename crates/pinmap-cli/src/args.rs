use std::path::PathBuf;

use clap::builder::PossibleValuesParser;
use clap::{Args, Parser, Subcommand, ValueEnum};
use pinmap_model::{Bias, DocKind, Drive, Format, Level, Mode, Speed};

/// A clap value enum mirroring a model enum, with kebab-case names.
macro_rules! value_enum {
    ($arg:ident => $model:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Clone, Copy, ValueEnum)]
        pub enum $arg {
            $($variant),+
        }

        impl From<$arg> for $model {
            fn from(value: $arg) -> Self {
                match value {
                    $($arg::$variant => $model::$variant),+
                }
            }
        }
    };
}

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
    /// Assign peripheral signals or pins.
    #[command(subcommand)]
    Assign(AssignCommand),
    /// Remove a pin's uses, a peripheral, or one PERIPH.SIGNAL.
    Unassign {
        /// A pin, a peripheral, or PERIPH.SIGNAL.
        name: String,
        #[command(flatten)]
        edit: EditFlags,
    },
    /// Set or remove devicetree properties.
    #[command(subcommand)]
    Prop(PropCommand),
    /// Set or clear a note on a pin entry or a peripheral.
    Note(NoteArgs),
    /// Generate output, such as a pinout document, from the project.
    Emit(EmitArgs),
    /// Show what this build of pinmap can do.
    #[command(subcommand)]
    List(ListCommand),
}

#[derive(Subcommand)]
pub enum ListCommand {
    /// The output formats `pinmap emit --to` accepts, with their options.
    Emitters,
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

value_enum!(KindArg => DocKind { Pindef, Pinasg });
value_enum!(FormatArg => Format { Json, Ron });

#[derive(Args)]
pub struct FmtArgs {
    /// Report files that would change instead of rewriting them. RON files
    /// with comments are skipped, never failed.
    #[arg(long)]
    pub check: bool,
    /// Files to format. Defaults to the project's files and its pindef.
    pub paths: Vec<PathBuf>,
}

value_enum!(BiasArg => Bias { Floating, PullUp, PullDown });
value_enum!(DriveArg => Drive { PushPull, OpenDrain });
value_enum!(SpeedArg => Speed { Low, Medium, High, VeryHigh });
value_enum!(LevelArg => Level { Low, High });
value_enum!(ModeArg => Mode { GpioIn, GpioOut, Analog, Reserved });

#[derive(Args)]
pub struct EditFlags {
    /// Show the change as a diff without writing.
    #[arg(long)]
    pub dry_run: bool,
    /// Write even if the edit adds errors.
    #[arg(long)]
    pub allow_errors: bool,
    /// Rewrite RON files even if that drops their comments.
    #[arg(long)]
    pub allow_comment_loss: bool,
    /// The project file that receives new entries. Defaults to the root.
    #[arg(long, value_name = "FILE")]
    pub file: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum AssignCommand {
    /// Assign peripheral signals to pins.
    Periph(AssignPeriphArgs),
    /// Create or update a pin entry (GPIO, analog, or reserved).
    Pin(AssignPinArgs),
}

#[derive(Args)]
pub struct AssignPeriphArgs {
    /// Peripheral name, for example SPI1.
    pub periph: String,
    /// Signals to assign.
    #[arg(required = true, value_name = "SIGNAL=PIN[:NET]")]
    pub signals: Vec<String>,
    /// Pull resistor for the listed signals.
    #[arg(long)]
    pub bias: Option<BiasArg>,
    /// Output driver for the listed signals.
    #[arg(long)]
    pub drive: Option<DriveArg>,
    /// Slew rate for the listed signals.
    #[arg(long)]
    pub speed: Option<SpeedArg>,
    #[command(flatten)]
    pub edit: EditFlags,
}

#[derive(Args)]
pub struct AssignPinArgs {
    /// Pin name, for example PA4.
    pub pin: String,
    /// Required for a pin that has no entry yet.
    #[arg(long)]
    pub mode: Option<ModeArg>,
    /// Output level at reset.
    #[arg(long)]
    pub initial: Option<LevelArg>,
    /// Pull resistor.
    #[arg(long)]
    pub bias: Option<BiasArg>,
    /// Net label in the schematic.
    #[arg(long)]
    pub net: Option<String>,
    #[command(flatten)]
    pub edit: EditFlags,
}

#[derive(Subcommand)]
pub enum PropCommand {
    /// Set a devicetree property, for example spi-max-frequency '<8000000>'.
    Set {
        periph: String,
        key: String,
        value: String,
        #[command(flatten)]
        edit: EditFlags,
    },
    /// Remove a devicetree property.
    Unset {
        periph: String,
        key: String,
        #[command(flatten)]
        edit: EditFlags,
    },
}

#[derive(Args)]
pub struct NoteArgs {
    /// A pin with an entry, or a peripheral.
    pub name: String,
    /// The note.
    #[arg(required_unless_present = "clear", conflicts_with = "clear")]
    pub text: Option<String>,
    /// Remove the note.
    #[arg(long)]
    pub clear: bool,
    #[command(flatten)]
    pub edit: EditFlags,
}

#[derive(Args)]
pub struct EmitArgs {
    /// The emitter to run (`pinmap list emitters` shows them).
    #[arg(long, value_parser = PossibleValuesParser::new(emitter_ids()))]
    pub to: String,
    /// Write the output file here instead of printing it.
    #[arg(short = 'o', long, value_name = "FILE", conflicts_with = "out_dir")]
    pub output: Option<PathBuf>,
    /// Write every output file into this directory.
    #[arg(long, value_name = "DIR")]
    pub out_dir: Option<PathBuf>,
    /// An emitter option, for example -O free-pins=true.
    #[arg(short = 'O', value_name = "KEY=VALUE", value_parser = parse_option)]
    pub options: Vec<(String, String)>,
    /// Emit even if the project has errors.
    #[arg(long)]
    pub allow_errors: bool,
}

fn emitter_ids() -> Vec<&'static str> {
    pinmap_emit::all().iter().map(|e| e.id()).collect()
}

fn parse_option(text: &str) -> Result<(String, String), String> {
    text.split_once('=')
        .filter(|(key, _)| !key.is_empty())
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .ok_or_else(|| format!("expected KEY=VALUE, got {text}"))
}
