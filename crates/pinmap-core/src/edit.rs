use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use pinmap_model::{
    Bias, Drive, Level, Mode, ModelError, PeriphAsg, PinAsg, PinId, SignalAsg, Speed, classify,
    has_comments, to_string, write_atomic,
};

use crate::{Diagnostic, Project, Severity, Usage, load, resolve};

/// One `SIGNAL=PIN[:NET]` to assign.
pub struct SignalSpec {
    pub signal: String,
    pub pin: PinId,
    pub net: Option<String>,
}

/// Electrical settings applied to every signal in one assignment. Fields left
/// `None` are not changed.
#[derive(Default)]
pub struct SignalConfig {
    pub bias: Option<Bias>,
    pub drive: Option<Drive>,
    pub speed: Option<Speed>,
}

/// Fields to set on a pin entry. Fields left `None` are not changed.
#[derive(Default)]
pub struct PinChange {
    pub mode: Option<Mode>,
    pub initial: Option<Level>,
    pub bias: Option<Bias>,
    pub net: Option<String>,
}

/// What [`Session::unassign`] removed.
#[derive(Debug, PartialEq, Eq)]
pub enum Removed {
    Pin { pin: PinId, uses: Vec<Usage> },
    Peripheral(String),
    Signal { periph: String, signal: String },
}

#[derive(Debug, Default, Clone, Copy)]
pub struct EditOptions {
    pub allow_errors: bool,
    pub allow_comment_loss: bool,
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: PathBuf,
    pub before: String,
    pub after: String,
}

#[derive(Debug)]
pub struct EditOutcome {
    /// Files whose canonical text changed.
    pub files: Vec<FileChange>,
    /// Diagnostics present after the edit but not before.
    pub added: Vec<Diagnostic>,
    /// Diagnostics present before the edit but not after.
    pub resolved: Vec<Diagnostic>,
    /// The edit adds errors and `allow_errors` was not set; nothing was written.
    pub rejected: bool,
    pub written: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum EditError {
    #[error("the project could not be loaded; run `pinmap check`")]
    Load(Vec<Diagnostic>),
    #[error("{} is not one of the project's files", .0.display())]
    NotInProject(PathBuf),
    #[error("{0} has no entry yet; give it a mode with --mode")]
    ModeRequired(PinId),
    #[error("{name} is not assigned{}", hint(.suggestion))]
    NotAssigned {
        name: String,
        suggestion: Option<String>,
    },
    #[error(
        "{0} names both an assigned peripheral and a pin in use; unassign PERIPH.SIGNAL entries one at a time"
    )]
    Ambiguous(String),
    #[error(
        "{0} is used only by a peripheral signal; notes go on the peripheral, or on a pin entry made with `pinmap assign pin`"
    )]
    NoteTarget(String),
    #[error(
        "{} has comments that rewriting would lose; use --allow-comment-loss to write anyway",
        .path.display()
    )]
    CommentLoss { path: PathBuf },
    #[error(transparent)]
    Model(#[from] ModelError),
}

fn hint(suggestion: &Option<String>) -> String {
    suggestion
        .as_ref()
        .map(|s| format!("; did you mean {s}?"))
        .unwrap_or_default()
}

/// A loaded project being edited in memory. Nothing touches disk until
/// [`Session::commit`].
pub struct Session {
    project: Project,
    load_diagnostics: Vec<Diagnostic>,
    before: Vec<Diagnostic>,
    /// The text of each project file as read, by index into `project.files`.
    originals: Vec<String>,
    touched: BTreeSet<usize>,
    /// Index of the file that receives new entries.
    new_entries: usize,
}

impl Session {
    /// Loads the project rooted at `root`. Fails only when there is no
    /// pindef to check edits against.
    pub fn open(root: &Path) -> Result<Self, EditError> {
        let (project, load_diagnostics) = load(root);
        if project.device.is_none() {
            return Err(EditError::Load(load_diagnostics));
        }
        let (_, resolve_diagnostics) = resolve(&project);
        let mut before = load_diagnostics.clone();
        before.extend(resolve_diagnostics);
        let originals = project
            .files
            .iter()
            .map(|f| {
                fs::read_to_string(&f.disk_path).map_err(|err| ModelError::At {
                    path: f.path.clone(),
                    source: Box::new(ModelError::Io(err)),
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            project,
            load_diagnostics,
            before,
            originals,
            touched: BTreeSet::new(),
            new_entries: 0,
        })
    }

    /// Adds or updates signals of `periph`, creating the peripheral if needed.
    /// An existing signal keeps its net unless a new one is given.
    pub fn assign_signals(
        &mut self,
        periph: &str,
        signals: &[SignalSpec],
        config: &SignalConfig,
    ) -> Result<(), EditError> {
        let i = self.periph_file(periph).unwrap_or(self.new_entries);
        let entry = self
            .doc_mut(i)
            .peripherals
            .entry(periph.to_owned())
            .or_insert_with(|| PeriphAsg {
                signals: IndexMap::new(),
                props: IndexMap::new(),
                note: None,
            });
        for spec in signals {
            let signal = entry
                .signals
                .entry(spec.signal.clone())
                .or_insert_with(|| SignalAsg {
                    pin: spec.pin.clone(),
                    net: None,
                    bias: None,
                    drive: None,
                    speed: None,
                });
            signal.pin = spec.pin.clone();
            if let Some(net) = &spec.net {
                signal.net = Some(net.clone());
            }
            if config.bias.is_some() {
                signal.bias = config.bias;
            }
            if config.drive.is_some() {
                signal.drive = config.drive;
            }
            if config.speed.is_some() {
                signal.speed = config.speed;
            }
        }
        Ok(())
    }

    /// Checks the edited project and, unless rejected or a dry run, writes
    /// every changed file.
    pub fn commit(self, options: &EditOptions) -> Result<EditOutcome, EditError> {
        let (_, resolve_after) = resolve(&self.project);
        let mut after = self.load_diagnostics.clone();
        after.extend(resolve_after);

        let key = |d: &Diagnostic| (d.code, d.file.clone(), d.path.clone());
        let before_keys: HashSet<_> = self.before.iter().map(key).collect();
        let after_keys: HashSet<_> = after.iter().map(key).collect();
        let added: Vec<Diagnostic> = after
            .iter()
            .filter(|d| !before_keys.contains(&key(d)))
            .cloned()
            .collect();
        let resolved: Vec<Diagnostic> = self
            .before
            .iter()
            .filter(|d| !after_keys.contains(&key(d)))
            .cloned()
            .collect();
        let rejected = !options.allow_errors && added.iter().any(|d| d.severity == Severity::Error);

        let mut changes = Vec::new();
        for &i in &self.touched {
            let file = &self.project.files[i];
            let (_, format) = classify(&file.path)?;
            let text = to_string(&file.doc, format)?;
            if text == self.originals[i] {
                continue;
            }
            if has_comments(&self.originals[i], format) && !options.allow_comment_loss {
                return Err(EditError::CommentLoss {
                    path: file.path.clone(),
                });
            }
            let change = FileChange {
                path: file.path.clone(),
                before: self.originals[i].clone(),
                after: text,
            };
            changes.push((i, change));
        }

        let written = !rejected && !options.dry_run && !changes.is_empty();
        if written {
            for (i, change) in &changes {
                write_atomic(&self.project.files[*i].disk_path, &change.after)?;
            }
        }
        Ok(EditOutcome {
            files: changes.into_iter().map(|(_, change)| change).collect(),
            added,
            resolved,
            rejected,
            written,
        })
    }

    fn doc_mut(&mut self, i: usize) -> &mut PinAsg {
        self.touched.insert(i);
        &mut self.project.files[i].doc
    }

    /// The first file defining peripheral `name`, matching how merging picks.
    fn periph_file(&self, name: &str) -> Option<usize> {
        self.project
            .files
            .iter()
            .position(|f| f.doc.peripherals.contains_key(name))
    }
}
