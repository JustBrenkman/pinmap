use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use pinmap_core::{Counts, EmitOptions, OutputFile, check_options};
use pinmap_model::write_atomic;
use serde::Serialize;

use crate::args::EmitArgs;
use crate::output::{Ctx, Exit, Render, render_diagnostic, render_errors};
use crate::project;

#[derive(Serialize)]
pub struct EmitReport {
    pub written: Vec<PathBuf>,
}

impl Render for EmitReport {
    fn render(&self, out: &mut String) {
        for path in &self.written {
            let _ = writeln!(out, "wrote {}", path.display());
        }
    }
}

pub fn run(ctx: &Ctx, args: &EmitArgs) -> anyhow::Result<Exit> {
    let emitters = pinmap_emit::all();
    let emitter = emitters
        .iter()
        .find(|e| e.id() == args.to)
        .context("unknown emitter")?;
    let options: EmitOptions = args.options.iter().cloned().collect();
    check_options(emitter.as_ref(), &options)?;

    let (resolved, diagnostics) = project::load(ctx)?;
    let Some(resolved) = resolved else {
        eprint!("{}", render_errors(&diagnostics));
        bail!("cannot load the project's pindef");
    };
    let count = Counts::of(&diagnostics).errors;
    if count > 0 && !args.allow_errors {
        eprint!("{}", render_errors(&diagnostics));
        eprintln!(
            "✘ not emitting: the project has {count} error(s) (use --allow-errors to emit anyway)"
        );
        return Ok(Exit::Problems);
    }

    let output = emitter.emit(&resolved, &options)?;
    for d in &output.unsupported {
        let mut text = String::new();
        render_diagnostic(d, &mut text);
        eprint!("{text}");
    }

    match (&args.output, &args.out_dir) {
        (None, None) => print!("{}", single(&args.to, &output.files)?.contents),
        (Some(path), _) => {
            write(path, &single(&args.to, &output.files)?.contents)?;
            ctx.emit(&EmitReport {
                written: vec![path.clone()],
            })?;
        }
        (None, Some(dir)) => {
            let mut written = Vec::new();
            for file in &output.files {
                let path = dir.join(&file.path);
                write(&path, &file.contents)?;
                written.push(path);
            }
            ctx.emit(&EmitReport { written })?;
        }
    }
    Ok(Exit::Ok)
}

/// The emitter's only file; several files need `--out-dir`.
fn single<'a>(id: &str, files: &'a [OutputFile]) -> anyhow::Result<&'a OutputFile> {
    match files {
        [file] => Ok(file),
        _ => bail!(
            "{id} writes {} files; choose a directory with --out-dir",
            files.len()
        ),
    }
}

fn write(path: &Path, contents: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
    }
    write_atomic(path, contents)?;
    Ok(())
}

#[derive(Serialize)]
pub struct OptionInfo {
    pub name: &'static str,
    pub help: &'static str,
    pub default: &'static str,
}

#[derive(Serialize)]
pub struct EmitterInfo {
    pub id: &'static str,
    pub description: &'static str,
    pub options: Vec<OptionInfo>,
}

#[derive(Serialize)]
pub struct EmitterList {
    pub emitters: Vec<EmitterInfo>,
}

impl Render for EmitterList {
    fn render(&self, out: &mut String) {
        for emitter in &self.emitters {
            let _ = writeln!(out, "{}  {}", emitter.id, emitter.description);
            for option in &emitter.options {
                let _ = writeln!(
                    out,
                    "    -O {}={}  {}",
                    option.name, option.default, option.help
                );
            }
        }
    }
}

pub fn list(ctx: &Ctx) -> anyhow::Result<Exit> {
    let emitters = pinmap_emit::all()
        .iter()
        .map(|e| EmitterInfo {
            id: e.id(),
            description: e.description(),
            options: e
                .options()
                .iter()
                .map(|o| OptionInfo {
                    name: o.name,
                    help: o.help,
                    default: o.default,
                })
                .collect(),
        })
        .collect();
    ctx.emit(&EmitterList { emitters })?;
    Ok(Exit::Ok)
}
