//! Starting libav, and the conversions every module of this crate needs from its types.

use crate::MediaError;
use anyview_core::{FilePath, MediaTime};
use ffmpeg_next as ff;
use std::path::Path;

/// Starts libav and quiets its log, which prints to stderr what a caller handles as an error.
/// libav's own initialisation runs once however often this is called.
pub(crate) fn start() -> Result<(), MediaError> {
    ff::init().map_err(libav)?;
    ff::util::log::set_level(ff::util::log::Level::Quiet);
    Ok(())
}

/// A libav failure as this crate's error.
pub(crate) fn libav(error: ff::Error) -> MediaError {
    MediaError::Libav(error.to_string())
}

/// `path` as text libav can take: its bindings accept only UTF-8 names.
pub(crate) fn utf8(path: &Path) -> Result<&str, MediaError> {
    path.to_str().ok_or_else(|| MediaError::Io {
        op: "read",
        path: path.to_path_buf(),
        kind: std::io::ErrorKind::InvalidInput,
    })
}

/// A path this crate made from a name it was given, as a `FilePath`.
pub(crate) fn file_path(path: &Path) -> Result<FilePath, MediaError> {
    FilePath::new(path).map_err(|_| MediaError::Io {
        op: "name",
        path: path.to_path_buf(),
        kind: std::io::ErrorKind::InvalidInput,
    })
}

/// A libav time in `base` units as microseconds, clamped at zero.
pub(crate) fn micros(value: i64, base: ff::Rational) -> MediaTime {
    let (num, den) = (i128::from(base.numerator()), i128::from(base.denominator()));
    if den == 0 {
        return MediaTime(0);
    }
    let scaled = i128::from(value) * num * 1_000_000 / den;
    MediaTime(u64::try_from(scaled.max(0)).unwrap_or(u64::MAX))
}

/// Microseconds as libav's `AV_TIME_BASE` units, which are the same.
pub(crate) fn av_time(time: MediaTime) -> i64 {
    i64::try_from(time.0).unwrap_or(i64::MAX)
}

/// A libav time in `base` units as signed microseconds: a packet's time can sit before zero.
pub(crate) fn signed_micros(value: i64, base: ff::Rational) -> i64 {
    let (num, den) = (i128::from(base.numerator()), i128::from(base.denominator()));
    if den == 0 {
        return 0;
    }
    i64::try_from(i128::from(value) * num * 1_000_000 / den).unwrap_or(i64::MAX)
}

/// Microseconds as a time in `base` units, rounded down.
pub(crate) fn in_base(micros: i64, base: ff::Rational) -> i64 {
    let (num, den) = (i128::from(base.numerator()), i128::from(base.denominator()));
    if num == 0 {
        return 0;
    }
    i64::try_from(i128::from(micros) * den / (num * 1_000_000)).unwrap_or(i64::MAX)
}
