//! What the file system says about one file besides its bytes: its dates, folder, permissions and
//! the address it was downloaded from. Best effort: a file system that has no birth time or no
//! extended attributes leaves that field absent.

use anyview_core::{ByteLen, Facts, FileDetails, Input, ModTime, Sniffed};
use rustix::fs::getxattr;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

/// The extended attribute browsers and download managers record a file's address in.
const ORIGIN_ATTRIBUTE: &str = "user.xdg.origin.url";

/// The most bytes of an address read: longer is not an address a person would read.
const ORIGIN_LIMIT: usize = 4096;

/// The details of the file at `path` (a link is followed). Blocking: it stats the file and reads
/// one extended attribute. A file that cannot be looked at has no details at all.
#[must_use]
pub fn file_details(path: &Path) -> FileDetails {
    let Ok(meta) = fs::metadata(path) else {
        return FileDetails::default();
    };
    FileDetails {
        len: Some(ByteLen(meta.len())),
        created: meta.created().ok().map(ModTime::from_system_time),
        modified: meta.modified().ok().map(ModTime::from_system_time),
        folder: path
            .parent()
            .filter(|folder| !folder.as_os_str().is_empty())
            .map(|folder| folder.display().to_string()),
        origin: origin_of(path),
        mode: Some(meta.mode() & 0o777),
    }
}

/// The General section of the file `input` is: its kind, size, dates, folder, where it came from
/// and permissions. Blocking, as [`file_details`] is. Bytes a host handed in, with no file of
/// their own, have only the kind and the size: there is no file whose dates or folder they could
/// be.
#[must_use]
pub fn general_facts(input: &Input, sniffed: &Sniffed) -> Facts {
    match input.path() {
        Some(path) => Facts::general(sniffed, &file_details(path.as_path())),
        None => Facts::general(
            sniffed,
            &FileDetails {
                len: Some(input.stamp().len),
                ..FileDetails::default()
            },
        ),
    }
}

fn origin_of(path: &Path) -> Option<String> {
    let mut value = vec![0_u8; ORIGIN_LIMIT];
    let length = getxattr(path, ORIGIN_ATTRIBUTE, value.as_mut_slice()).ok()?;
    let text = String::from_utf8(value[..length].to_vec()).ok()?;
    let text = text.trim_matches(|c: char| c == '\0' || c.is_whitespace());
    (!text.is_empty()).then(|| text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustix::fs::{XattrFlags, setxattr};
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn a_file_reports_its_size_dates_folder_and_mode() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        fs::write(&path, b"hello").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        let details = file_details(&path);
        assert_eq!(details.len, Some(ByteLen(5)));
        assert_eq!(details.mode, Some(0o640));
        assert_eq!(
            details.folder.as_deref(),
            Some(dir.path().to_str().unwrap())
        );
        assert!(details.modified.is_some());
        assert_eq!(details.origin, None);
    }

    #[test]
    fn the_download_address_is_read_from_the_extended_attribute() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.pdf");
        fs::write(&path, b"x").unwrap();
        let url = "https://example.com/a.pdf?token=1";
        if setxattr(&path, ORIGIN_ATTRIBUTE, url.as_bytes(), XattrFlags::empty()).is_err() {
            // This file system keeps no user attributes (a tmpfs without them): nothing to read.
            eprintln!("skipped: no user xattrs here");
            return;
        }
        assert_eq!(file_details(&path).origin.as_deref(), Some(url));
    }

    #[test]
    fn bytes_with_no_file_have_only_a_kind_and_a_size() {
        use anyview_core::{FileHead, FileName, FilePath, FileStamp, SniffStep, sniff};
        let name = FileName::new("a.txt").unwrap();
        let SniffStep::Done(sniffed) = sniff(&FileHead::new(b"hello"), &name) else {
            panic!("a text file is not a zip");
        };
        let stamp = FileStamp {
            len: ByteLen(5),
            modified: ModTime(0),
        };
        let bytes = Input::new(name, stamp, std::sync::Arc::new(b"hello".to_vec()));
        let facts = general_facts(&bytes, &sniffed);
        let rows: Vec<_> = facts
            .rows()
            .iter()
            .map(|row| (row.label, row.value.as_str()))
            .collect();
        assert_eq!(
            rows,
            [
                (anyview_core::FactLabel::Kind, "Plain text"),
                (anyview_core::FactLabel::Size, "5 B"),
            ]
        );

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.txt");
        fs::write(&path, b"hello").unwrap();
        let at_path = bytes.at(FilePath::new(&path).unwrap());
        let facts = general_facts(&at_path, &sniffed);
        assert!(!facts.rows().is_empty());
    }

    #[test]
    fn a_missing_file_has_no_details() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            file_details(&dir.path().join("gone")),
            FileDetails::default()
        );
    }
}
