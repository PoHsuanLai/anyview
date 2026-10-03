//! A file's `file://` URI, which the thumbnail spec hashes and the file manager and the
//! portals take.

use anyview_core::FilePath;
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_encode};
use std::os::unix::ffi::OsStrExt;

/// Everything but the unreserved characters and the separator is escaped, as the thumbnail
/// spec's reference implementation does.
const ESCAPED: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'/')
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

/// `path` as an absolute `file:///...` URI with its bytes percent-encoded.
pub fn file_uri(path: &FilePath) -> String {
    // Bytes, not `str`: a path need not be UTF-8.
    let encoded = percent_encode(path.as_path().as_os_str().as_bytes(), ESCAPED);
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
