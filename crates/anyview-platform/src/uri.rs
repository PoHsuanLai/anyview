//! A file's `file://` URI, which the thumbnail spec hashes and the file manager and the
//! portals take.

use anyview_core::FilePath;
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_encode};
use std::borrow::Cow;
use std::path::Path;

/// Everything but the unreserved characters and the separator is escaped, as the thumbnail
/// spec's reference implementation does.
const ESCAPED: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'/')
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

/// What is escaped on this platform: a drive letter's colon stays as it is on Windows.
#[cfg(windows)]
const ESCAPED_HERE: &AsciiSet = &ESCAPED.remove(b':');
#[cfg(not(windows))]
const ESCAPED_HERE: &AsciiSet = ESCAPED;

/// The path as the bytes of the URI's path part. A Unix path is bytes and need not be UTF-8.
#[cfg(unix)]
fn path_bytes(path: &Path) -> Cow<'_, [u8]> {
    use std::os::unix::ffi::OsStrExt;
    Cow::Borrowed(path.as_os_str().as_bytes())
}

/// The path as the bytes of the URI's path part: `C:\a b` is `/C:/a b`, as Windows programs write
/// a file URI (`file:///C:/a%20b`).
#[cfg(not(unix))]
fn path_bytes(path: &Path) -> Cow<'_, [u8]> {
    let text = path.to_string_lossy().replace('\\', "/");
    if text.starts_with('/') {
        Cow::Owned(text.into_bytes())
    } else {
        Cow::Owned(format!("/{text}").into_bytes())
    }
}

/// `path` as an absolute `file:///...` URI with its bytes percent-encoded.
pub fn file_uri(path: &FilePath) -> String {
    let bytes = path_bytes(path.as_path());
    let encoded = percent_encode(&bytes, ESCAPED_HERE);
    format!("file://{encoded}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_becomes_an_escaped_file_uri() {
        const CASES: &[(&str, &str, &str)] = &[
            ("plain", "/home/a/b.png", "file:///home/a/b.png"),
            ("space", "/home/a b/c.png", "file:///home/a%20b/c.png"),
            ("unicode", "/home/é.png", "file:///home/%C3%A9.png"),
            ("hash and percent", "/a/#1%.txt", "file:///a/%231%25.txt"),
            ("unreserved kept", "/a/x_y-z~.tar", "file:///a/x_y-z~.tar"),
        ];
        for (name, path, want) in CASES {
            let path = FilePath::new(path).unwrap();
            assert_eq!(file_uri(&path), *want, "{name}");
        }
    }
}
