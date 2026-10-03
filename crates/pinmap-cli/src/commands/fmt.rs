use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::bail;
use pinmap_model::{AnyDoc, classify, has_comments};
use serde::Serialize;

use crate::args::FmtArgs;
use crate::output::{Ctx, Exit, Render};
use crate::project;

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum FmtStatus {
    Unchanged,
    Reformatted,
    WouldChange,
    /// Left alone: rewriting would drop its comments.
    SkippedComments,
    Error {
        message: String,
    },
}

#[derive(Serialize)]
pub struct FmtFile {
    pub path: PathBuf,
    #[serde(flatten)]
    pub status: FmtStatus,
}

#[derive(Serialize)]
pub struct FmtReport {
    pub check: bool,
    pub files: Vec<FmtFile>,
}

impl Render for FmtReport {
    fn render(&self, out: &mut String) {
        let mut changed = 0;
        for file in &self.files {
            let path = file.path.display();
            match &file.status {
                FmtStatus::Unchanged => {}
                FmtStatus::Reformatted => {
                    changed += 1;
                    let _ = writeln!(out, "reformatted {path}");
                }
                FmtStatus::WouldChange => {
                    changed += 1;
                    let _ = writeln!(out, "would reformat {path}");
                }
                FmtStatus::SkippedComments => {
                    let _ = writeln!(
                        out,
                        "skipped {path}: has comments, which formatting would remove"
                    );
                }
                FmtStatus::Error { message } => {
                    let _ = writeln!(out, "error: {path}: {message}");
                }
            }
        }
        let verb = if self.check {
            "need formatting"
        } else {
            "reformatted"
        };
        let _ = writeln!(out, "{} files checked, {changed} {verb}", self.files.len());
    }
}

pub fn run(ctx: &Ctx, args: &FmtArgs) -> anyhow::Result<Exit> {
    let paths = if args.paths.is_empty() {
        project_files(ctx)?
    } else {
        args.paths.clone()
    };
    let files: Vec<FmtFile> = paths
        .into_iter()
        .map(|path| {
            let status = format_file(&path, args.check);
            FmtFile { path, status }
        })
        .collect();
    // `WouldChange` only occurs with `--check`.
    let failed = files
        .iter()
        .any(|f| matches!(f.status, FmtStatus::Error { .. } | FmtStatus::WouldChange));
    ctx.emit(&FmtReport {
        check: args.check,
        files,
    })?;
    Ok(if failed { Exit::Problems } else { Exit::Ok })
}

/// The root, its imports, and its pindef. Files that do not load are left
/// out; `pinmap check` reports them.
fn project_files(ctx: &Ctx) -> anyhow::Result<Vec<PathBuf>> {
    let (project, _) = pinmap_core::load(&project::root(ctx)?);
    let mut paths: Vec<PathBuf> = project.files.into_iter().map(|f| f.path).collect();
    paths.extend(project.device.map(|d| d.path));
    if paths.is_empty() {
        bail!("the project could not be loaded; run `pinmap check`");
    }
    Ok(paths)
}

fn format_file(path: &Path, check: bool) -> FmtStatus {
    match try_format(path, check) {
        Ok(status) => status,
        Err(err) => FmtStatus::Error {
            message: format!("{err:#}"),
        },
    }
}

fn try_format(path: &Path, check: bool) -> anyhow::Result<FmtStatus> {
    let (kind, format) = classify(path)?;
    let text = fs::read_to_string(path)?;
    let canonical = AnyDoc::from_str(&text, kind, format)?.to_string(format)?;
    Ok(if text == canonical {
        FmtStatus::Unchanged
    } else if has_comments(&text, format) {
        FmtStatus::SkippedComments
    } else if check {
        FmtStatus::WouldChange
    } else {
        pinmap_model::write_atomic(path, &canonical)?;
        FmtStatus::Reformatted
    })
}
