use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use pinmap_model::{DocKind, PinAsg, classify, normalize_path, read, resolve_ref};

#[derive(Debug, thiserror::Error)]
pub enum DiscoverError {
    #[error("no *.pinasg.json or *.pinasg.ron file under {}", .dir.display())]
    NoRoot { dir: PathBuf },

    #[error(
        "every pinasg file under {} is imported by another; choose the root with --project",
        .dir.display()
    )]
    AllImported { dir: PathBuf },

    #[error(
        "found {} root pinasg files; choose one with --project:\n  {}",
        .roots.len(),
        list(.roots)
    )]
    ManyRoots { roots: Vec<PathBuf> },

    #[error("cannot read {}: {error}", .dir.display())]
    Io { dir: PathBuf, error: std::io::Error },
}

fn list(paths: &[PathBuf]) -> String {
    let shown: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
    shown.join("\n  ")
}

/// Finds the root pinasg under `dir`: the one file no other file imports.
pub fn discover_root(dir: &Path) -> Result<PathBuf, DiscoverError> {
    let mut candidates = Vec::new();
    walk(dir, &mut candidates)?;
    if candidates.is_empty() {
        return Err(DiscoverError::NoRoot {
            dir: dir.to_path_buf(),
        });
    }
    candidates.sort();

    let mut imported = HashSet::new();
    for candidate in &candidates {
        // A file that does not parse stays a candidate, so it is not hidden.
        let Ok(doc) = read::<PinAsg>(candidate) else {
            continue;
        };
        let base = candidate.parent().unwrap_or(Path::new(""));
        for import in &doc.imports {
            if let Ok(found) = resolve_ref(base, import, DocKind::Pinasg)
                && let Ok(key) = fs::canonicalize(found)
            {
                imported.insert(key);
            }
        }
    }

    let mut roots: Vec<PathBuf> = candidates
        .into_iter()
        .filter(|c| fs::canonicalize(c).map_or(true, |key| !imported.contains(&key)))
        .map(|c| normalize_path(&c))
        .collect();
    match roots.len() {
        0 => Err(DiscoverError::AllImported {
            dir: dir.to_path_buf(),
        }),
        1 => Ok(roots.remove(0)),
        _ => Err(DiscoverError::ManyRoots { roots }),
    }
}

/// Walks the starting directory, which must be readable.
fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), DiscoverError> {
    let entries = fs::read_dir(dir).map_err(|error| DiscoverError::Io {
        dir: dir.to_path_buf(),
        error,
    })?;
    visit_entries(entries, out);
    Ok(())
}

/// Walks a subdirectory; one that cannot be read is skipped.
fn walk_below(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        visit_entries(entries, out);
    }
}

fn visit_entries(entries: fs::ReadDir, out: &mut Vec<PathBuf>) {
    for entry in entries.flatten() {
        let path = entry.path();
        // Does not follow symlinks, so symlinked directories are not walked.
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with('.') && name != "target" {
                walk_below(&path, out);
            }
        } else if (kind.is_file() || (kind.is_symlink() && path.is_file()))
            && matches!(classify(&path), Ok((DocKind::Pinasg, _)))
        {
            out.push(path);
        }
    }
}
