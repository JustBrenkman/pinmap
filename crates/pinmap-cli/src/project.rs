use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use pinmap_core::{Counts, Diagnostic, Resolved, Severity, discover_root, open};

use crate::output::{Ctx, render_diagnostic};

/// The root pinasg: `--project`, or the one found under the current directory.
pub fn root(ctx: &Ctx) -> anyhow::Result<PathBuf> {
    match &ctx.project {
        Some(path) => Ok(path.clone()),
        None => discover_root(Path::new(".")).context("cannot find the project"),
    }
}

/// Loads and checks the project.
pub fn load(ctx: &Ctx) -> anyhow::Result<(Option<Resolved>, Vec<Diagnostic>)> {
    Ok(open(&root(ctx)?))
}

/// Loads the project for inspection. Errors in the project are noted on
/// stderr; only a missing pindef stops the command.
pub fn load_resolved(ctx: &Ctx) -> anyhow::Result<Resolved> {
    let (resolved, diagnostics) = load(ctx)?;
    let Some(resolved) = resolved else {
        let mut text = String::new();
        for d in diagnostics.iter().filter(|d| d.severity == Severity::Error) {
            render_diagnostic(d, &mut text);
        }
        eprint!("{text}");
        bail!("cannot load the project's pindef");
    };
    let errors = Counts::of(&diagnostics).errors;
    if errors > 0 {
        eprintln!("note: project has {errors} error(s); run `pinmap check`");
    }
    Ok(resolved)
}
