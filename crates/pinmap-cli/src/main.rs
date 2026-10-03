//! The `pinmap` binary.

mod args;
mod commands;
mod output;
mod project;

use std::process::ExitCode;

use clap::Parser;

use args::{AssignCommand, Cli, Command, ListCommand, PeriphCommand, PinCommand};
use output::{Ctx, Exit};

fn main() -> ExitCode {
    let cli = Cli::parse();
    let ctx = Ctx {
        json: cli.json,
        project: cli.project,
    };
    let result = match &cli.command {
        Command::Check(args) => commands::check::run(&ctx, args),
        Command::Pin(PinCommand::List(args)) => commands::pin::list(&ctx, args),
        Command::Pin(PinCommand::Show { pin }) => commands::pin::show(&ctx, pin),
        Command::Periph(PeriphCommand::List) => commands::periph::list(&ctx),
        Command::Periph(PeriphCommand::Show { periph }) => commands::periph::show(&ctx, periph),
        Command::Convert(args) => commands::convert::run(&ctx, args),
        Command::Fmt(args) => commands::fmt::run(&ctx, args),
        Command::Schema { kind } => commands::schema::run(*kind),
        Command::Assign(AssignCommand::Periph(args)) => commands::edit::assign_periph(&ctx, args),
        Command::Assign(AssignCommand::Pin(args)) => commands::edit::assign_pin(&ctx, args),
        Command::Unassign { name, edit } => commands::edit::unassign(&ctx, name, edit),
        Command::Prop(command) => commands::edit::prop(&ctx, command),
        Command::Note(args) => commands::edit::note(&ctx, args),
        Command::Emit(args) => commands::emit::run(&ctx, args),
        Command::List(ListCommand::Emitters) => commands::emit::list(&ctx),
    };
    match result {
        Ok(exit) => exit.into(),
        Err(err) => {
            eprintln!("error: {err:#}");
            Exit::Failed.into()
        }
    }
}
