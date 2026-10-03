use std::fs;
use std::path::{Component, Path, PathBuf};

/// Removes `.` and folds `..` into the previous component, without touching
/// the file system. Meant for showing paths: compare files by canonical path.
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match out.components().next_back() {
                Some(Component::Normal(_)) => {
                    out.pop();
                }
                Some(Component::RootDir | Component::Prefix(_)) => {}
                _ => out.push(".."),
            },
            other => out.push(other),
        }
    }
    if out.as_os_str().is_empty() {
        out.push(".");
    }
    out
}

/// Whether two paths name the same file: compares canonical paths when both
/// exist, the paths themselves otherwise.
pub fn same_file(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_dots() {
        let cases = [
            ("a/../b/c", "b/c"),
            ("./a/./b", "a/b"),
            ("../x/y", "../x/y"),
            ("a/../../x", "../x"),
            ("/abs/../x", "/x"),
            ("a/..", "."),
        ];
        for (input, expected) in cases {
            assert_eq!(
                normalize_path(Path::new(input)),
                Path::new(expected),
                "{input}"
            );
        }
    }
}
