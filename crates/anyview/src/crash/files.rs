//! Crash reports on disk: one file each under `<state>/anyview/crash`, the newest [`KEEP`] kept.

use super::Report;
use anyview_store::STORE_FOLDER;
use std::io;
use std::path::{Path, PathBuf};

/// The folder under the program's state folder that holds the reports.
pub const CRASH_FOLDER: &str = "crash";

/// How many reports are kept; older ones are removed when a new one is written.
pub const KEEP: usize = 10;

/// Where the reports go: `<state>/anyview/crash`.
pub fn crash_dir(state: &Path) -> PathBuf {
    state.join(STORE_FOLDER).join(CRASH_FOLDER)
}

/// Writes `report` into `dir` and prunes it to the newest [`KEEP`]. The name is the time, padded so
/// that names sort in time order, and a counter when two reports share a second. Returns the file.
pub fn write_report(dir: &Path, report: &Report) -> io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let mut counter = 0u32;
    let (path, mut file) = loop {
        let path = dir.join(format!("{:012}-{counter:03}.txt", report.at));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => break (path, file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists && counter < 999 => {
                counter += 1;
            }
            Err(error) => return Err(error),
        }
    };
    io::Write::write_all(&mut file, report.to_string().as_bytes())?;
    prune(dir)?;
    Ok(path)
}

/// Removes all but the newest [`KEEP`] reports in `dir`; other files are left alone.
fn prune(dir: &Path) -> io::Result<()> {
    let mut names: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "txt"))
        .collect();
    names.sort();
    let surplus = names.len().saturating_sub(KEEP);
    for old in names.into_iter().take(surplus) {
        std::fs::remove_file(old)?;
    }
    Ok(())
}
