use std::path::{Path, PathBuf};

/// The repository's `fixtures` directory.
pub fn fixtures() -> PathBuf {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    crates.parent().unwrap().join("fixtures")
}
