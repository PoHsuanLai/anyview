//! The panic hook.

use super::{Report, write_report};
use std::panic::{PanicHookInfo, set_hook};
use std::path::PathBuf;

/// Installs the hook: every panic prints a short report to stderr and writes it into `dir`. `now`
/// is the time in seconds since the epoch. Writing is best effort: a full disk never makes a panic
/// worse.
pub fn install(dir: PathBuf, version: &'static str, now: impl Fn() -> u64 + Send + Sync + 'static) {
    set_hook(Box::new(move |info| {
        let report = report_of(info, version, now());
        eprint!("{report}");
        match write_report(&dir, &report) {
            Ok(path) => eprintln!("report: {}", path.display()),
            Err(error) => eprintln!("report not saved: {error}"),
        }
    }));
}

fn report_of(info: &PanicHookInfo<'_>, version: &str, at: u64) -> Report {
    let payload = info.payload();
    let message = payload
        .downcast_ref::<&str>()
        .map(|text| (*text).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "(no message)".to_owned());
    Report {
        thread: std::thread::current()
            .name()
            .unwrap_or("<unnamed>")
            .to_owned(),
        message,
        location: info
            .location()
            .map_or_else(|| "unknown".to_owned(), ToString::to_string),
        version: version.to_owned(),
        at,
    }
}
