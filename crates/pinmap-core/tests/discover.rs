use std::fs;
use std::path::{Path, PathBuf};

use pinmap_core::{DiscoverError, discover_root};

/// A pinasg that imports `imports`. The device never has to exist: discovery
/// reads only `imports`.
fn pinasg(imports: &[&str]) -> String {
    let imports: Vec<String> = imports.iter().map(|i| format!("{i:?}")).collect();
    format!(
        r#"{{"schema": "pinmap/v1", "kind": "pinasg", "device": "chip",
            "target": {{"package": "P"}}, "imports": [{}]}}"#,
        imports.join(", ")
    )
}

fn project(files: &[(&str, String)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (name, text) in files {
        let path = dir.path().join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    dir
}

fn relative(dir: &tempfile::TempDir, paths: &[PathBuf]) -> Vec<PathBuf> {
    paths
        .iter()
        .map(|p| p.strip_prefix(dir.path()).unwrap().to_path_buf())
        .collect()
}

#[test]
fn finds_the_file_nothing_imports() {
    let dir = project(&[
        ("board/main.pinasg.json", pinasg(&["usb"])),
        ("board/usb.pinasg.ron", pinasg(&[])),
    ]);
    let root = discover_root(dir.path()).unwrap();
    assert_eq!(
        relative(&dir, &[root]),
        [Path::new("board/main.pinasg.json")]
    );
}

#[test]
fn several_roots_are_listed_sorted() {
    let dir = project(&[
        ("b.pinasg.json", pinasg(&[])),
        ("a.pinasg.json", pinasg(&[])),
    ]);
    let Err(DiscoverError::ManyRoots { roots }) = discover_root(dir.path()) else {
        panic!()
    };
    assert_eq!(
        relative(&dir, &roots),
        [Path::new("a.pinasg.json"), Path::new("b.pinasg.json")]
    );
}

#[test]
fn no_pinasg_at_all() {
    let dir = project(&[("chip.pindef.json", "{}".into())]);
    assert!(matches!(
        discover_root(dir.path()),
        Err(DiscoverError::NoRoot { .. })
    ));
}

#[test]
fn every_file_imported_is_a_cycle_not_a_root() {
    let dir = project(&[
        ("a.pinasg.json", pinasg(&["b"])),
        ("b.pinasg.json", pinasg(&["a"])),
    ]);
    assert!(matches!(
        discover_root(dir.path()),
        Err(DiscoverError::AllImported { .. })
    ));
}

#[test]
fn unparseable_candidates_stay_visible() {
    let dir = project(&[
        ("main.pinasg.json", pinasg(&[])),
        ("broken.pinasg.json", "{".into()),
    ]);
    let Err(DiscoverError::ManyRoots { roots }) = discover_root(dir.path()) else {
        panic!()
    };
    assert_eq!(roots.len(), 2);
}

#[test]
fn skips_hidden_and_target_directories() {
    let dir = project(&[
        ("main.pinasg.json", pinasg(&[])),
        (".git/old.pinasg.json", pinasg(&[])),
        ("target/copy.pinasg.json", pinasg(&[])),
    ]);
    let root = discover_root(dir.path()).unwrap();
    assert_eq!(relative(&dir, &[root]), [Path::new("main.pinasg.json")]);
}

#[test]
fn relative_start_gives_clean_relative_paths() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/json");
    let root = discover_root(&fixtures).unwrap();
    assert!(
        root.ends_with("board/revb.pinasg.json"),
        "{}",
        root.display()
    );
    assert!(!root.to_string_lossy().contains(".."), "{}", root.display());
}
