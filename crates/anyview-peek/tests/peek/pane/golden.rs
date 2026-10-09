//! Golden files, hand-rolled: compare a rendered string with `tests/snapshots/<name>`.
//!
//! `DS_BLESS=1 cargo test ...` writes the actual output instead of comparing. Read the diff
//! before committing a bless: a golden that was rewritten to match is not evidence of anything.
//! Copied from quire's `ds/tests/support/golden.rs`, minus the directory scan.

use std::path::PathBuf;

/// Where the golden called `name` lives: `crates/<crate>/tests/snapshots/<name>`.
pub fn path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshots")
        .join(name)
}

/// Whether this run rewrites goldens (`DS_BLESS=1`).
fn blessing() -> bool {
    std::env::var("DS_BLESS").is_ok_and(|value| value == "1")
}

/// Compare `actual` with the golden `name`, or rewrite it under `DS_BLESS=1`.
///
/// Returns the failure as text instead of panicking, so a table-driven test can report every
/// case that differs at once.
pub fn check(name: &str, actual: &str) -> Result<(), String> {
    let file = path(name);
    let actual = format!("{actual}\n");
    if blessing() {
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{name}: {e}"))?;
        }
        return std::fs::write(&file, actual).map_err(|e| format!("{name}: {e}"));
    }
    match std::fs::read_to_string(&file) {
        Ok(expected) if expected == actual => Ok(()),
        Ok(expected) => Err(format!(
            "{name} differs\n  golden: {}\n  actual: {}",
            expected.trim_end(),
            actual.trim_end()
        )),
        Err(e) => Err(format!(
            "{name}: {e} (run with DS_BLESS=1 to write it, then read it)"
        )),
    }
}
