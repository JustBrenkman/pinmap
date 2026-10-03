use std::fmt::Write as _;
use std::path::PathBuf;

use anyhow::anyhow;
use pinmap_core::{
    Diagnostic, EditError, EditOptions, PinChange, Session, Severity, SignalConfig, SignalSpec,
};
use serde::Serialize;
use similar::TextDiff;

use crate::args::{AssignPeriphArgs, AssignPinArgs, EditFlags, NoteArgs, PropCommand};
use crate::output::{Ctx, Exit, Render, render_diagnostic};
use crate::project;

#[derive(Serialize)]
pub struct FileDiff {
    pub path: PathBuf,
    pub diff: String,
}

#[derive(Serialize)]
pub struct EditReport {
    #[serde(skip)]
    dry_run: bool,
    pub files: Vec<FileDiff>,
    pub added: Vec<Diagnostic>,
    pub resolved: Vec<Diagnostic>,
    pub rejected: bool,
    pub written: bool,
}

impl Render for EditReport {
    fn render(&self, out: &mut String) {
        for file in &self.files {
            if self.dry_run || self.rejected {
                out.push_str(&file.diff);
            } else {
                let _ = writeln!(out, "updated {}", file.path.display());
            }
        }
        if !self.added.is_empty() {
            out.push_str("new:\n");
            for d in &self.added {
                render_diagnostic(d, out);
            }
        }
        if !self.resolved.is_empty() {
            out.push_str("resolved:\n");
            for d in &self.resolved {
                let _ = writeln!(out, "  {} {}", d.code, d.message);
            }
        }
        let n = self.files.len();
        let files = if n == 1 { "file" } else { "files" };
        let summary = if self.rejected {
            let errors = self
                .added
                .iter()
                .filter(|d| d.severity == Severity::Error)
                .count();
            let s = if errors == 1 { "" } else { "s" };
            format!("✘ rejected: {errors} new error{s} (use --allow-errors to write anyway)")
        } else if n == 0 {
            "✔ no changes".to_owned()
        } else if self.dry_run {
            format!("✔ dry run: would update {n} {files}")
        } else {
            format!("✔ updated {n} {files}")
        };
        let _ = writeln!(out, "{summary}");
    }
}

/// Opens the project, applies `change`, commits, and prints the result.
pub fn run_edit(
    ctx: &Ctx,
    flags: &EditFlags,
    change: impl FnOnce(&mut Session) -> Result<(), EditError>,
) -> anyhow::Result<Exit> {
    let mut session = Session::open(&project::root(ctx)?).inspect_err(print_load_errors)?;
    if let Some(file) = &flags.file {
        session.set_new_entry_file(file)?;
    }
    change(&mut session)?;
    let options = EditOptions {
        allow_errors: flags.allow_errors,
        allow_comment_loss: flags.allow_comment_loss,
        dry_run: flags.dry_run,
    };
    let outcome = session.commit(&options)?;
    let files = outcome
        .files
        .iter()
        .map(|f| {
            let shown = f.path.display().to_string();
            let diff = TextDiff::from_lines(&f.before, &f.after)
                .unified_diff()
                .context_radius(3)
                .header(&shown, &shown)
                .to_string();
            FileDiff {
                path: f.path.clone(),
                diff,
            }
        })
        .collect();
    let report = EditReport {
        dry_run: flags.dry_run,
        files,
        added: outcome.added,
        resolved: outcome.resolved,
        rejected: outcome.rejected,
        written: outcome.written,
    };
    ctx.emit(&report)?;
    Ok(if report.rejected {
        Exit::Problems
    } else {
        Exit::Ok
    })
}

fn print_load_errors(err: &EditError) {
    if let EditError::Load(diagnostics) = err {
        let mut text = String::new();
        for d in diagnostics.iter().filter(|d| d.severity == Severity::Error) {
            render_diagnostic(d, &mut text);
        }
        eprint!("{text}");
    }
}

/// Parses `SIGNAL=PIN` or `SIGNAL=PIN:NET`.
fn parse_spec(text: &str) -> anyhow::Result<SignalSpec> {
    let bad = || anyhow!("expected SIGNAL=PIN[:NET], got {text}");
    let (signal, rest) = text.split_once('=').ok_or_else(bad)?;
    let (pin, net) = match rest.split_once(':') {
        Some((pin, net)) => (pin, Some(net)),
        None => (rest, None),
    };
    if signal.is_empty() || pin.is_empty() || net.is_some_and(str::is_empty) {
        return Err(bad());
    }
    Ok(SignalSpec {
        signal: signal.into(),
        pin: pin.into(),
        net: net.map(str::to_owned),
    })
}

pub fn assign_periph(ctx: &Ctx, args: &AssignPeriphArgs) -> anyhow::Result<Exit> {
    let specs = args
        .signals
        .iter()
        .map(|s| parse_spec(s))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let config = SignalConfig {
        bias: args.bias.map(Into::into),
        drive: args.drive.map(Into::into),
        speed: args.speed.map(Into::into),
    };
    run_edit(ctx, &args.edit, |s| {
        s.assign_signals(&args.periph, &specs, &config)
    })
}

pub fn assign_pin(ctx: &Ctx, args: &AssignPinArgs) -> anyhow::Result<Exit> {
    let change = PinChange {
        mode: args.mode.map(Into::into),
        initial: args.initial.map(Into::into),
        bias: args.bias.map(Into::into),
        net: args.net.clone(),
    };
    run_edit(ctx, &args.edit, |s| {
        s.assign_pin(&args.pin.as_str().into(), &change)
    })
}

pub fn unassign(ctx: &Ctx, name: &str, flags: &EditFlags) -> anyhow::Result<Exit> {
    run_edit(ctx, flags, |s| s.unassign(name).map(|_| ()))
}

pub fn prop(ctx: &Ctx, command: &PropCommand) -> anyhow::Result<Exit> {
    match command {
        PropCommand::Set {
            periph,
            key,
            value,
            edit,
        } => run_edit(ctx, edit, |s| s.set_prop(periph, key, value)),
        PropCommand::Unset { periph, key, edit } => {
            run_edit(ctx, edit, |s| s.unset_prop(periph, key))
        }
    }
}

pub fn note(ctx: &Ctx, args: &NoteArgs) -> anyhow::Result<Exit> {
    let text = if args.clear {
        None
    } else {
        args.text.as_deref()
    };
    run_edit(ctx, &args.edit, |s| s.set_note(&args.name, text))
}
