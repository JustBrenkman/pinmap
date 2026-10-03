use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use pinmap_model::{DocKind, ModelError, PinAsg, PinDef, Target, read, resolve_ref};

use crate::Diagnostic;
use crate::paths::normalize;

#[derive(Debug, Clone)]
pub struct LoadedPinDef {
    pub path: PathBuf,
    pub doc: PinDef,
}

#[derive(Debug, Clone)]
pub struct LoadedPinAsg {
    pub path: PathBuf,
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
    let root = normalize(root);
    let mut loader = Loader::default();
    loader.visit(&root, None);
    let project = Project {
        root,
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
    /// `referrer` is the file and logical path that imported `path`.
    fn visit(&mut self, path: &Path, referrer: Option<(&Path, String)>) {
        let key = fs::canonicalize(path).ok();
        if let Some(key) = &key {
            if let Some(start) = self.stack.iter().position(|(k, _)| k == key) {
                let mut chain: Vec<String> = self.stack[start..]
                    .iter()
                    .map(|(_, shown)| shown.display().to_string())
                    .collect();
                chain.push(path.display().to_string());
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

        let doc: PinAsg = match read(path) {
            Ok(doc) => doc,
            Err(err) => {
                self.diagnostics.push(Diagnostic::from_model(&err));
                return;
            }
        };
        let dir = path.parent().unwrap_or(Path::new("")).to_path_buf();

        let device = match resolve_ref(&dir, &doc.device, DocKind::Pindef) {
            Ok(found) => Some(normalize(&found)),
            Err(err) => {
                self.diagnostics
                    .push(reference_error(&err, path, "device".into()));
                None
            }
        };
        if self.root.is_none() {
            self.root = Some(RootInfo {
                device: device.clone(),
                target: doc.target.clone(),
            });
            if let Some(device) = device {
                self.load_device(device);
            }
        } else {
            self.check_matches_root(path, device.as_deref(), &doc.target);
        }

        let imports = doc.imports.clone();
        self.files.push(LoadedPinAsg {
            path: path.to_path_buf(),
            doc,
        });
        self.stack.push((
            key.unwrap_or_else(|| path.to_path_buf()),
            path.to_path_buf(),
        ));
        for (i, import) in imports.iter().enumerate() {
            let at = format!("imports[{i}]");
            match resolve_ref(&dir, import, DocKind::Pinasg) {
                Ok(found) => self.visit(&normalize(&found), Some((path, at))),
                Err(err) => self.diagnostics.push(reference_error(&err, path, at)),
            }
        }
        self.stack.pop();
    }

    fn load_device(&mut self, path: PathBuf) {
        match read::<PinDef>(&path) {
            Ok(doc) => self.device = Some(LoadedPinDef { path, doc }),
            Err(err) => self.diagnostics.push(Diagnostic::from_model(&err)),
        }
    }

    fn check_matches_root(&mut self, path: &Path, device: Option<&Path>, target: &Target) {
        let Some(root) = &self.root else { return };
        if let (Some(mine), Some(theirs)) = (device, &root.device)
            && !same_file(mine, theirs)
        {
            let message = format!(
                "device is {}, but the root uses {}",
                mine.display(),
                theirs.display()
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
