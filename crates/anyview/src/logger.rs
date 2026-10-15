//! The program's log: `LEVEL target: message` lines on stderr, which a launcher sends to the
//! journal. Libraries only call the `log` macros; this is the one place that decides where the
//! lines go.

use log::{LevelFilter, Log, Metadata, Record};
use std::ffi::OsStr;

/// The environment variable that sets the level: `error`, `warn`, `info`, `debug`, `trace` or
/// `off`.
pub const LEVEL_VARIABLE: &str = "ANYVIEW_LOG";

/// What is logged when the variable is not set or names no level.
const DEFAULT: LevelFilter = LevelFilter::Warn;

/// Writes each line that passes `level` to stderr.
struct Stderr {
    level: LevelFilter,
}

impl Log for Stderr {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record<'_>) {
        if self.enabled(record.metadata()) {
            eprintln!("{} {}: {}", record.level(), record.target(), record.args());
        }
    }

    fn flush(&self) {}
}

/// The level `text` names, ignoring case; the default for nothing or for a word that is no level.
pub fn level_of(text: Option<&OsStr>) -> LevelFilter {
    text.and_then(OsStr::to_str)
        .and_then(|text| text.trim().parse().ok())
        .unwrap_or(DEFAULT)
}

/// Send the `log` macros to stderr at the level `ANYVIEW_LOG` asks (`warn` without it). A second
/// call changes nothing.
pub fn install() {
    let level = level_of(std::env::var_os(LEVEL_VARIABLE).as_deref());
    if log::set_boxed_logger(Box::new(Stderr { level })).is_ok() {
        log::set_max_level(level);
    }
}
