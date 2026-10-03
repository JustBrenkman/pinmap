use pinmap_core::{Counts, Diagnostic};
use serde::Serialize;

use crate::args::CheckArgs;
use crate::output::{Ctx, Exit, Render, render_diagnostic};
use crate::project;

#[derive(Serialize)]
pub struct CheckReport {
    pub diagnostics: Vec<Diagnostic>,
    pub summary: Counts,
}

impl Render for CheckReport {
    fn render(&self, out: &mut String) {
        for d in &self.diagnostics {
            render_diagnostic(d, out);
        }
        let Counts {
            errors,
            warnings,
            infos,
        } = self.summary;
        let parts: Vec<String> = [(errors, "error"), (warnings, "warning"), (infos, "info")]
            .into_iter()
            .filter(|(n, _)| *n > 0)
            .map(|(n, word)| {
                let plural = n != 1 && word != "info";
                format!("{n} {word}{}", if plural { "s" } else { "" })
            })
            .collect();
        let mark = if errors > 0 { '✘' } else { '✔' };
        let text = if parts.is_empty() {
            "no problems".to_owned()
        } else {
            parts.join(", ")
        };
        out.push_str(&format!("{mark} {text}\n"));
    }
}

pub fn run(ctx: &Ctx, args: &CheckArgs) -> anyhow::Result<Exit> {
    let (_, diagnostics) = project::load(ctx)?;
    let summary = Counts::of(&diagnostics);
    ctx.emit(&CheckReport {
        diagnostics,
        summary,
    })?;
    let failed = summary.errors > 0 || (args.deny_warnings && summary.warnings > 0);
    Ok(if failed { Exit::Problems } else { Exit::Ok })
}
