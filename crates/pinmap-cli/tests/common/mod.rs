#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

pub fn fixtures() -> PathBuf {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    crates.parent().unwrap().join("fixtures")
}

pub struct Run {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Runs the `pinmap` binary in `dir`.
pub fn pinmap(dir: &Path, args: &[&str]) -> Run {
    let output = Command::new(env!("CARGO_BIN_EXE_pinmap"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    Run {
        code: output.status.code().unwrap(),
        stdout: unixify(&String::from_utf8(output.stdout).unwrap()),
        stderr: unixify(&String::from_utf8(output.stderr).unwrap()),
    }
}

/// Path separators as on Unix, so snapshots match on every platform.
fn unixify(text: &str) -> String {
    if cfg!(windows) {
        text.replace("\\\\", "/").replace('\\', "/")
    } else {
        text.to_owned()
    }
}
