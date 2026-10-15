//! Leftovers of a process that died while writing: the hidden temporary files a save or an
//! export makes beside a person's file, named for the process that made them.

use crate::sys::is_running;
use std::fs;
use std::path::Path;

/// Deletes the temporary files beside a file whose process is gone: `.<name>.anyview-<pid>[-<n>].tmp`
/// from a save and `.<name>.<pid>-<n>.part` from an export or a copy. A file of a process that is
/// still running is not touched. Best effort: a folder that cannot be read has none to remove.
/// Returns how many went.
pub fn sweep_leftovers(dir: &Path) -> usize {
    sweep_where(dir, leftover_of)
}

/// Deletes the scratch files in `dir` called `<prefix>-<pid>-<n>.<extension>` whose process is gone:
/// the files a trim or a frame export of a viewer that was killed left in its cache folder. A
/// file of a process that is still running is not touched. Returns how many went.
pub fn sweep_scratch(dir: &Path, prefix: &str) -> usize {
    sweep_where(dir, |name| scratch_of(name, prefix))
}

/// Deletes the files in `dir` whose name `owner_of` says a process made, if that process is gone.
fn sweep_where(dir: &Path, owner_of: impl Fn(&str) -> Option<i32>) -> usize {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name.to_str().and_then(&owner_of) else {
            continue;
        };
        let is_file = entry.file_type().is_ok_and(|kind| kind.is_file());
        if is_file && !is_running(pid) && fs::remove_file(entry.path()).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// The process that made the scratch file called `name`, or `None` when it is not one of `prefix`'s.
fn scratch_of(name: &str, prefix: &str) -> Option<i32> {
    let rest = name.strip_prefix(prefix)?.strip_prefix('-')?;
    let (tail, _extension) = rest.split_once('.')?;
    leading_pid(tail, TailRule::NumberRequired)
}

/// The process that made the temporary file called `name`, or `None` when it is not one.
fn leftover_of(name: &str) -> Option<i32> {
    let rest = name.strip_prefix('.')?;
    if let Some(body) = rest.strip_suffix(".tmp") {
        let (_, tail) = body.rsplit_once(".anyview-")?;
        return leading_pid(tail, TailRule::NumberOptional);
    }
    let body = rest.strip_suffix(".part")?;
    let (_, tail) = body.rsplit_once('.')?;
    leading_pid(tail, TailRule::NumberRequired)
}

#[derive(Clone, Copy)]
enum TailRule {
    /// `<pid>` or `<pid>-<n>`.
    NumberOptional,
    /// `<pid>-<n>`.
    NumberRequired,
}

fn leading_pid(tail: &str, rule: TailRule) -> Option<i32> {
    let (pid, number) = match tail.split_once('-') {
        Some((pid, number)) => (pid, Some(number)),
        None => (tail, None),
    };
    match (rule, number) {
        (TailRule::NumberRequired, None) => return None,
        (_, Some(number)) if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) => {
            return None;
        }
        (TailRule::NumberRequired | TailRule::NumberOptional, _) => {}
    }
    if pid.is_empty() || !pid.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    pid.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CASES: &[(&str, &str, Option<i32>)] = &[
        (
            "a save's temporary file",
            ".doc.txt.anyview-4242-7.tmp",
            Some(4242),
        ),
        (
            "a save's old temporary file",
            ".doc.txt.anyview-4242.tmp",
            Some(4242),
        ),
        (
            "an export's partial file",
            ".out.png.4242-3.part",
            Some(4242),
        ),
        ("an export's old partial file", ".out.png.4242.part", None),
        ("a person's hidden file", ".bashrc", None),
        ("a person's tmp file", ".notes.tmp", None),
        ("not a number", ".a.anyview-x-1.tmp", None),
        ("no dot", "doc.anyview-1-1.tmp", None),
    ];

    #[test]
    fn a_leftover_names_the_process_that_made_it() {
        for (name, file, want) in CASES {
            assert_eq!(leftover_of(file), *want, "{name}");
        }
    }

    #[test]
    fn a_scratch_file_names_the_process_that_made_it() {
        const SCRATCH: &[(&str, &str, &str, Option<i32>)] = &[
            ("a trim", "trim", "trim-4242-3.mkv", Some(4242)),
            ("a frame", "frame", "frame-4242-0.png", Some(4242)),
            ("another prefix", "trim", "frame-4242-0.png", None),
            ("no number", "trim", "trim-4242.mkv", None),
            ("not a pid", "trim", "trim-x-1.mkv", None),
        ];
        for (name, prefix, file, want) in SCRATCH {
            assert_eq!(scratch_of(file, prefix), *want, "{name}");
        }
    }
}
