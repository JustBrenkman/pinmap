use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use pinmap_model::{
    DocKind, ModelError, PinAsg, PinDef, Target, normalize_path, read_from, resolve_ref,
};

use crate::Diagnostic;

#[derive(Debug, Clone)]
pub struct LoadedPinDef {
    /// The path as shown to the user, with `.` and `..` folded.
    pub path: PathBuf,
    /// Where the file is read from and written to: the canonical path.
    pub disk_path: PathBuf,
    pub doc: PinDef,
}

#[derive(Debug, Clone)]
pub struct LoadedPinAsg {
    /// The path as shown to the user, with `.` and `..` folded.
    pub path: PathBuf,
    /// Where the file is read from and written to: the canonical path.
    pub disk_path: PathBuf,
    pub doc: PinAsg,
}

/// A root pinasg and everything it references, as read from disk.
#[derive(Debug, Clone, Default)]
pub struct Project {
    pub root: PathBuf,
    /// `None` when the root or its pindef could not be loaded.
    pub device: Option<LoadedPinDef>,
    /// The root first, then imports in the order they were reached.
    pub files: Vec<LoadedPinAsg>,
}

/// Loads a root pinasg, its pindef, and its imports. Problems become
/// diagnostics, and loading carries on with whatever could be read.
pub fn load(root: &Path) -> (Project, Vec<Diagnostic>) {
    let shown = normalize_path(root);
    let mut loader = Loader::default();
    loader.visit(&shown, root, None);
    let project = Project {
        root: shown,
        device: loader.device,
        files: loader.files,
    };
    (project, loader.diagnostics)
}

/// The root's pindef path and target, which every import must match.
struct RootInfo {
    device: Option<PathBuf>,
    target: Target,
}

#[derive(Default)]
struct Loader {
    files: Vec<LoadedPinAsg>,
    device: Option<LoadedPinDef>,
    root: Option<RootInfo>,
    /// Canonical paths of files already visited.
    seen: HashSet<PathBuf>,
    /// The chain of imports being followed, as (canonical, shown) paths.
    stack: Vec<(PathBuf, PathBuf)>,
    diagnostics: Vec<Diagnostic>,
}

impl Loader {
    /// Visits a pinasg shown as `shown` and found on disk at `real`.
    /// `referrer` is the file and logical path that imported it.
    fn visit(&mut self, shown: &Path, real: &Path, referrer: Option<(&Path, String)>) {
        let key = fs::canonicalize(real).ok();
        if let Some(key) = &key {
            if let Some(start) = self.stack.iter().position(|(k, _)| k == key) {
                let mut chain: Vec<String> = self.stack[start..]
                    .iter()
                    .map(|(_, shown)| shown.display().to_string())
                    .collect();
                chain.push(shown.display().to_string());
                let message = format!("import cycle: {}", chain.join(" -> "));
                let mut diag = Diagnostic::error("PM0107", message);
                if let Some((file, at)) = referrer {
                    diag = diag.in_file(file).at_path(at);
                }
                self.diagnostics.push(diag);
                return;
            }
            if !self.seen.insert(key.clone()) {
                return;
            }
        }

        let disk_path = key.unwrap_or_else(|| real.to_path_buf());
        let doc: PinAsg = match read_from(real, &disk_path) {
            Ok(doc) => doc,
            Err(err) => {
                self.diagnostics
                    .push(Diagnostic::from_model(&err).in_file(shown));
                return;
            }
        };
        let dir = real.parent().unwrap_or(Path::new("")).to_path_buf();

        let is_root = self.root.is_none();
        let device = match resolve_ref(&dir, &doc.device, DocKind::Pindef) {
            Ok(found) => Some(found),
            Err(err) => {
                // A missing pindef is reported once, at the root.
                let root_has_device = self.root.as_ref().is_some_and(|r| r.device.is_some());
                if is_root || root_has_device {
                    self.diagnostics
                        .push(reference_error(&err, shown, "device".into()));
                }
                None
            }
        };
        if is_root {
            self.root = Some(RootInfo {
                device: device.clone(),
                target: doc.target.clone(),
            });
            if let Some(device) = device {
                self.load_device(&device);
            }
        } else {
            self.check_matches_root(shown, device.as_deref(), &doc.target);
        }

        let imports = doc.imports.clone();
        self.files.push(LoadedPinAsg {
            path: shown.to_path_buf(),
            disk_path: disk_path.clone(),
            doc,
        });
        self.stack.push((disk_path, shown.to_path_buf()));
        for (i, import) in imports.iter().enumerate() {
            let at = format!("imports[{i}]");
            match resolve_ref(&dir, import, DocKind::Pinasg) {
                Ok(found) => self.visit(&normalize_path(&found), &found, Some((shown, at))),
                Err(err) => self.diagnostics.push(reference_error(&err, shown, at)),
            }
        }
        self.stack.pop();
    }

    fn load_device(&mut self, real: &Path) {
        let shown = normalize_path(real);
        let disk_path = fs::canonicalize(real).unwrap_or_else(|_| real.to_path_buf());
        match read_from::<PinDef>(real, &disk_path) {
            Ok(doc) => {
                self.device = Some(LoadedPinDef {
                    path: shown,
                    disk_path,
                    doc,
                })
            }
            Err(err) => self
                .diagnostics
                .push(Diagnostic::from_model(&err).in_file(shown)),
        }
    }

    fn check_matches_root(&mut self, path: &Path, device: Option<&Path>, target: &Target) {
        let Some(root) = &self.root else { return };
        if let (Some(mine), Some(theirs)) = (device, &root.device)
            && !same_file(mine, theirs)
        {
            let message = format!(
                "device is {}, but the root uses {}",
                normalize_path(mine).display(),
                normalize_path(theirs).display()
            );
            self.diagnostics.push(
                Diagnostic::error("PM0109", message)
                    .in_file(path)
                    .at_path("device"),
            );
        }
        if *target != root.target {
            let message = format!(
                "target is {}, but the root uses {}",
                describe(target),
                describe(&root.target)
            );
            self.diagnostics.push(
                Diagnostic::error("PM0109", message)
                    .in_file(path)
                    .at_path("target"),
            );
        }
    }
}

/// A `device` or `imports` reference that could not be resolved, reported
/// where it was written.
fn reference_error(err: &ModelError, file: &Path, at: String) -> Diagnostic {
    Diagnostic::error(err.code(), err.to_string())
        .in_file(file)
        .at_path(at)
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

fn describe(target: &Target) -> String {
    match &target.refdes {
        Some(refdes) => format!("{} ({refdes})", target.package),
        None => target.package.clone(),
    }
}
