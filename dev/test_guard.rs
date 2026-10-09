//! Keeps every crate's integration tests inside the one binary of `tests/<name>/main.rs`.
//!
//! Cargo makes a binary, and a full link of the dev-dependency tree, of every `tests/*.rs` file
//! and every `tests/<dir>/main.rs`. A new `tests/topic.rs` written by habit would quietly be one
//! more. Each integration binary includes this file with
//! `#[path = "../../../../dev/test_guard.rs"] mod test_guard;` and calls [`check`] from one test.

use std::fs;
use std::path::Path;

/// Fails if `tests/` of the crate at `manifest_dir` holds anything but the binary `name`.
///
/// - `own_process`: the stems of the `tests/*.rs` files that must run in a process of their own
///   (a panic hook, a high-water mark of memory, a timing budget). Each must exist.
/// - `data_dirs`: the directories of `tests/` that hold data and no code (fixtures, goldens).
/// - `main_rs`: the text of the binary's `main.rs`, from `include_str!("main.rs")`.
///
/// Every `.rs` file and every directory with a `mod.rs` in `tests/<name>/` must be declared there
/// with a `mod` line.
pub fn check(
    manifest_dir: &str,
    name: &str,
    own_process: &[&str],
    data_dirs: &[&str],
    main_rs: &str,
) {
    let tests = Path::new(manifest_dir).join("tests");
    let mut stray = Vec::new();
    for entry in fs::read_dir(&tests).expect("the tests directory") {
        let path = entry.expect("a directory entry").path();
        let file = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let stem = path.file_stem().and_then(|n| n.to_str()).unwrap_or("");
        let is_rs = path.extension().is_some_and(|ext| ext == "rs");
        if path.is_file() && is_rs && !own_process.contains(&stem) {
            stray.push(file.to_owned());
        } else if path.is_dir() && file != name && !data_dirs.contains(&file) {
            stray.push(format!("{file}/"));
        }
    }
    assert!(
        stray.is_empty(),
        "these would be binaries of their own; move them into tests/{name}/ as modules, or, if \
         one must run in its own process, name it in `own_process`: {stray:?}"
    );
    for stem in own_process {
        assert!(
            tests.join(format!("{stem}.rs")).is_file(),
            "`own_process` names tests/{stem}.rs, which is not there"
        );
    }

    let declared = |module: &str| {
        ["mod", "pub mod", "pub(crate) mod"].iter().any(|keyword| {
            main_rs
                .lines()
                .any(|line| line.trim() == format!("{keyword} {module};"))
        })
    };
    let mut unlisted = Vec::new();
    for entry in fs::read_dir(tests.join(name)).expect("the binary's directory") {
        let path = entry.expect("a directory entry").path();
        let Some(module) = path.file_stem().and_then(|n| n.to_str()) else {
            continue;
        };
        let is_module = if path.is_dir() {
            path.join("mod.rs").is_file()
        } else {
            path.extension().is_some_and(|ext| ext == "rs")
        };
        if is_module && module != "main" && !declared(module) {
            unlisted.push(module.to_owned());
        }
    }
    assert!(
        unlisted.is_empty(),
        "tests/{name}/main.rs declares no `mod` for: {unlisted:?}"
    );
}
