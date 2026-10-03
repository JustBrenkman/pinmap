//! The `pinmap` binary.

mod args;
mod commands;
mod output;
mod project;

use std::process::ExitCode;

use clap::Parser;

use args::{Cli, Command};
use output::{Ctx, Exit};

fn main() -> ExitCode {
    let cli = Cli::parse();
    let ctx = Ctx {
        json: cli.json,
        project: cli.project,
    };
    let result = match &cli.command {
        Command::Check(args) => commands::check::run(&ctx, args),
    };
    match result {
        Ok(exit) => exit.into(),
        Err(err) => {
            eprintln!("error: {err:#}");
            Exit::Failed.into()
        }
    }
}
