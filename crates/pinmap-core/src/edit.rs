use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use pinmap_model::{
    Bias, Drive, Level, Mode, ModelError, PinAsg, PinAsgEntry, PinId, SignalAsg, Speed, classify,
    has_comments, stage, to_string,
};

use crate::{Diagnostic, Project, Severity, Usage, list_or_none, load, resolve};

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
    #[error("{source}; already updated: {}", list_or_none(.updated.iter().map(|p| p.display())))]
    PartialWrite {
        updated: Vec<PathBuf>,
        source: ModelError,
    },
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

    /// Sends new peripherals and pin entries to `file`, which must be one of
    /// the project's pinasg files.
    pub fn set_new_entry_file(&mut self, file: &Path) -> Result<(), EditError> {
        let wanted =
            fs::canonicalize(file).map_err(|_| EditError::NotInProject(file.to_path_buf()))?;
        self.new_entries = self
            .project
            .files
            .iter()
            .position(|f| f.disk_path == wanted)
            .ok_or_else(|| EditError::NotInProject(file.to_path_buf()))?;
        Ok(())
    }

    /// Creates or updates the entry for `pin`. A new entry needs a mode.
    pub fn assign_pin(&mut self, pin: &PinId, change: &PinChange) -> Result<(), EditError> {
        let i = self.pin_file(pin.as_str()).unwrap_or(self.new_entries);
        let exists = self.project.files[i].doc.pins.contains_key(pin);
        if !exists && change.mode.is_none() {
            return Err(EditError::ModeRequired(pin.clone()));
        }
        let entry = self
            .doc_mut(i)
            .pins
            .entry(pin.clone())
            .or_insert_with(|| PinAsgEntry {
                mode: change.mode.unwrap_or(Mode::GpioIn),
                initial: None,
                bias: None,
                net: None,
                note: None,
            });
        entry.mode = change.mode.unwrap_or(entry.mode);
        entry.initial = change.initial.or(entry.initial);
        entry.bias = change.bias.or(entry.bias);
        entry.net = change.net.clone().or(entry.net.take());
        Ok(())
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
            .or_default();
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
            signal.net = spec.net.clone().or(signal.net.take());
            signal.bias = config.bias.or(signal.bias);
            signal.drive = config.drive.or(signal.drive);
            signal.speed = config.speed.or(signal.speed);
        }
        Ok(())
    }

    /// Removes `PERIPH.SIGNAL`, a whole peripheral, or everything on a pin.
    /// A peripheral left with no signals, props, or note is removed too.
    pub fn unassign(&mut self, name: &str) -> Result<Removed, EditError> {
        if let Some((periph, signal)) = name.split_once('.') {
            let i = self
                .periph_file(periph)
                .filter(|&i| {
                    self.project.files[i].doc.peripherals[periph]
                        .signals
                        .contains_key(signal)
                })
                .ok_or_else(|| self.not_assigned(name))?;
            if let Some(p) = self.doc_mut(i).peripherals.get_mut(periph) {
                p.signals.shift_remove(signal);
            }
            self.prune(i, periph);
            return Ok(Removed::Signal {
                periph: periph.into(),
                signal: signal.into(),
            });
        }
        let periph = self.periph_file(name);
        let uses = self.pin_uses(name);
        match (periph, uses.is_empty()) {
            (Some(_), false) => Err(EditError::Ambiguous(name.into())),
            (Some(_), true) => {
                self.remove_peripheral(name);
                Ok(Removed::Peripheral(name.into()))
            }
            (None, false) => {
                for (i, usage) in &uses {
                    match usage {
                        Usage::Signal { periph, signal } => {
                            if let Some(p) = self.doc_mut(*i).peripherals.get_mut(periph) {
                                p.signals.shift_remove(signal);
                            }
                            self.prune(*i, periph);
                        }
                        Usage::Pin { .. } => {
                            self.doc_mut(*i).pins.shift_remove(name);
                        }
                    }
                }
                Ok(Removed::Pin {
                    pin: name.into(),
                    uses: uses.into_iter().map(|(_, usage)| usage).collect(),
                })
            }
            (None, true) => Err(self.not_assigned(name)),
        }
    }

    /// Sets a devicetree property on an assigned peripheral.
    pub fn set_prop(&mut self, periph: &str, key: &str, value: &str) -> Result<(), EditError> {
        let i = self
            .periph_file(periph)
            .ok_or_else(|| self.not_assigned(periph))?;
        if let Some(p) = self.doc_mut(i).peripherals.get_mut(periph) {
            p.props.insert(key.to_owned(), value.to_owned());
        }
        Ok(())
    }

    /// Removes a devicetree property from an assigned peripheral.
    pub fn unset_prop(&mut self, periph: &str, key: &str) -> Result<(), EditError> {
        let i = self
            .periph_file(periph)
            .ok_or_else(|| self.not_assigned(periph))?;
        if !self.project.files[i].doc.peripherals[periph]
            .props
            .contains_key(key)
        {
            return Err(EditError::NotAssigned {
                name: format!("{periph} property {key}"),
                suggestion: None,
            });
        }
        if let Some(p) = self.doc_mut(i).peripherals.get_mut(periph) {
            p.props.shift_remove(key);
        }
        self.prune(i, periph);
        Ok(())
    }

    /// Sets or, with `None`, clears the note on a peripheral or a pin entry.
    pub fn set_note(&mut self, name: &str, note: Option<&str>) -> Result<(), EditError> {
        let note = note.map(str::to_owned);
        if let Some(i) = self.periph_file(name) {
            if let Some(p) = self.doc_mut(i).peripherals.get_mut(name) {
                p.note = note;
            }
            self.prune(i, name);
            return Ok(());
        }
        if let Some(i) = self.pin_file(name) {
            if let Some(entry) = self.doc_mut(i).pins.get_mut(name) {
                entry.note = note;
            }
            return Ok(());
        }
        if !self.pin_uses(name).is_empty() {
            return Err(EditError::NoteTarget(name.into()));
        }
        Err(self.not_assigned(name))
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
            // Stage every file before renaming any, so a failed write leaves
            // the project as it was. Dropped stages delete their temp files.
            let staged = changes
                .iter()
                .map(|(i, change)| stage(&self.project.files[*i].disk_path, &change.after))
                .collect::<Result<Vec<_>, _>>()?;
            let mut updated = Vec::new();
            for (staged, (_, change)) in staged.into_iter().zip(&changes) {
                staged.commit().map_err(|source| EditError::PartialWrite {
                    updated: updated.clone(),
                    source,
                })?;
                updated.push(change.path.clone());
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

    /// Every use of `pin` across files, with the index of the file holding it.
    fn pin_uses(&self, pin: &str) -> Vec<(usize, Usage)> {
        let mut uses = Vec::new();
        for (i, file) in self.project.files.iter().enumerate() {
            for (periph, p) in &file.doc.peripherals {
                for (signal, s) in &p.signals {
                    if s.pin.as_str() == pin {
                        let usage = Usage::Signal {
                            periph: periph.clone(),
                            signal: signal.clone(),
                        };
                        uses.push((i, usage));
                    }
                }
            }
            if let Some(entry) = file.doc.pins.get(pin) {
                uses.push((i, Usage::Pin { mode: entry.mode }));
            }
        }
        uses
    }

    /// `NotAssigned` for `name`, suggesting the nearest assigned name.
    fn not_assigned(&self, name: &str) -> EditError {
        let mut names: Vec<String> = Vec::new();
        for file in &self.project.files {
            for (periph, p) in &file.doc.peripherals {
                names.push(periph.clone());
                for (signal, s) in &p.signals {
                    names.push(format!("{periph}.{signal}"));
                    names.push(s.pin.to_string());
                }
            }
            names.extend(file.doc.pins.keys().map(PinId::to_string));
        }
        let suggestion = crate::suggest(name, names.iter().map(String::as_str)).map(str::to_owned);
        EditError::NotAssigned {
            name: name.into(),
            suggestion,
        }
    }

    /// Drops `periph` once file `i`'s definition has nothing left: no
    /// signals, props, or note.
    fn prune(&mut self, i: usize, periph: &str) {
        let empty = self.project.files[i]
            .doc
            .peripherals
            .get(periph)
            .is_some_and(|p| p.signals.is_empty() && p.props.is_empty() && p.note.is_none());
        if empty {
            self.remove_peripheral(periph);
        }
    }

    /// Removes `periph` from every file, so that a duplicate definition
    /// (PM0108) does not take the removed one's place.
    fn remove_peripheral(&mut self, periph: &str) {
        for i in 0..self.project.files.len() {
            if self.project.files[i].doc.peripherals.contains_key(periph) {
                self.doc_mut(i).peripherals.shift_remove(periph);
            }
        }
    }

    fn pin_file(&self, pin: &str) -> Option<usize> {
        self.project
            .files
            .iter()
            .position(|f| f.doc.pins.contains_key(pin))
    }
}
