use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow, bail};
use pinmap_model::{AnyDoc, classify, has_comments};
use serde::Serialize;

use crate::args::ConvertArgs;
use crate::output::{Ctx, Exit, Render};

#[derive(Serialize)]
pub struct ConvertReport {
    pub input: PathBuf,
    pub output: PathBuf,
    pub removed_input: bool,
    /// Both formats now sit side by side under one stem.
    pub ambiguous_stem: bool,
}

impl Render for ConvertReport {
    fn render(&self, out: &mut String) {
        out.push_str(&format!("wrote {}\n", self.output.display()));
        if self.removed_input {
            out.push_str(&format!("removed {}\n", self.input.display()));
        }
        if self.ambiguous_stem {
            out.push_str(&format!(
                "note: {} still exists, so references to its stem are ambiguous until one copy is removed\n",
                self.input.display()
            ));
        }
    }
}

pub fn run(ctx: &Ctx, args: &ConvertArgs) -> anyhow::Result<Exit> {
    if args.input == Path::new("-") {
        return from_stdin(args);
    }
    let input = &args.input;
    let output = args
        .output
        .as_ref()
        .context("missing OUTPUT file (use `-` as INPUT to write to stdout)")?;
    let (in_kind, in_format) = classify(input)?;
    let (out_kind, out_format) = classify(output)?;
    if in_kind != out_kind {
        bail!("cannot convert a {in_kind} into a {out_kind}");
    }
    if same_file(input, output) {
        bail!("input and output are the same file");
    }
    if output.exists() && !args.force {
        bail!("{} exists; use --force to overwrite it", output.display());
    }
    let text =
        fs::read_to_string(input).with_context(|| format!("cannot read {}", input.display()))?;
    if args.rm_source && has_comments(&text, in_format) {
        bail!(
            "{} has comments, which conversion would lose; convert without --rm-source and move them by hand",
            input.display()
        );
    }
    let doc = AnyDoc::from_str(&text, in_kind, in_format)
        .map_err(|e| anyhow!("{}: {e}", input.display()))?;
    fs::write(output, doc.to_string(out_format)?)
        .with_context(|| format!("cannot write {}", output.display()))?;
    if args.rm_source {
        fs::remove_file(input).with_context(|| format!("cannot remove {}", input.display()))?;
    }
    ctx.emit(&ConvertReport {
        input: input.clone(),
        output: output.clone(),
        removed_input: args.rm_source,
        ambiguous_stem: !args.rm_source && stem(input) == stem(output),
    })?;
    Ok(Exit::Ok)
}

fn from_stdin(args: &ConvertArgs) -> anyhow::Result<Exit> {
    let (Some(kind), Some(from), Some(to)) = (args.kind, args.from, args.to) else {
        bail!("reading stdin needs --kind, --from and --to");
    };
    if args.output.as_deref().is_some_and(|o| o != Path::new("-")) {
        bail!("with stdin input the result goes to stdout; omit OUTPUT");
    }
    let mut text = String::new();
    io::stdin().read_to_string(&mut text)?;
    let doc = AnyDoc::from_str(&text, kind.into(), from.into())?;
    print!("{}", doc.to_string(to.into())?);
    Ok(Exit::Ok)
}

fn same_file(a: &Path, b: &Path) -> bool {
    a == b || matches!((fs::canonicalize(a), fs::canonicalize(b)), (Ok(x), Ok(y)) if x == y)
}

/// The path without its `.<kind>.<format>` ending.
fn stem(path: &Path) -> Option<PathBuf> {
    let name = path.file_name()?.to_str()?;
    let stem = name.rsplitn(3, '.').nth(2)?;
    Some(path.with_file_name(stem))
}
