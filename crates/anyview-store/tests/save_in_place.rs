//! The save pipeline and the versions store through the public API, in scratch directories.

// Helpers in an integration test crate are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

use anyview_store::{DEFAULT_KEEP, KeepPeriod, Pending, SavedAt, StoreError, Versions};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const DAY: u64 = 86_400;

/// A scratch folder with a versions store and one file, `doc.txt`, holding `original`.
struct Scratch {
    _dir: tempfile::TempDir,
    versions: Versions,
    file: PathBuf,
}

fn scratch(original: &str) -> Scratch {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("work").join("doc.txt");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, original).unwrap();
    let versions = Versions::under_state(&dir.path().join("state"));
    Scratch {
        _dir: dir,
        versions,
        file,
    }
}

fn save(scratch: &Scratch, bytes: &str, at: u64) {
    scratch
        .versions
        .back_up(Pending::new(&scratch.file, bytes.into()), SavedAt(at))
        .unwrap()
        .write_in_place()
        .unwrap();
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

#[test]
fn a_save_replaces_the_file_and_keeps_the_original() {
    let s = scratch("before");
    let backed = s
        .versions
        .back_up(Pending::new(&s.file, b"after".to_vec()), SavedAt(100))
        .unwrap();
    assert_eq!(
        read(&s.file),
        "before",
        "backing up must not touch the file"
    );
    let written = backed.write_in_place().unwrap();

    assert_eq!(read(&s.file), "after");
    let versions = s.versions.list(&s.file).unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].id, written.kept);
    assert_eq!(versions[0].saved_at, SavedAt(100));
    assert_eq!(versions[0].size.0, "before".len() as u64);
    assert_eq!(versions[0].path, fs::canonicalize(&s.file).unwrap());
}

/// (name, mode of the original)
const MODES: &[(&str, u32)] = &[
    ("private file", 0o600),
    ("group readable", 0o640),
    ("executable", 0o755),
];

#[test]
fn a_save_keeps_the_originals_mode_and_leaves_no_temporary_file() {
    for (name, mode) in MODES {
        let s = scratch("before");
        fs::set_permissions(&s.file, fs::Permissions::from_mode(*mode)).unwrap();
        save(&s, "after", 1);

        let got = fs::metadata(&s.file).unwrap().permissions().mode() & 0o777;
        assert_eq!(got, *mode, "{name}");
        let names: Vec<_> = fs::read_dir(s.file.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["doc.txt"], "{name}: a temporary file was left");
    }
}

#[test]
fn a_failed_write_leaves_the_original_untouched_and_no_temporary_file() {
    let s = scratch("before");
    let backed = s
        .versions
        .back_up(Pending::new(&s.file, b"after".to_vec()), SavedAt(5))
        .unwrap();
    let folder = s.file.parent().unwrap();
    fs::set_permissions(folder, fs::Permissions::from_mode(0o555)).unwrap();
    let result = backed.write_in_place();
    fs::set_permissions(folder, fs::Permissions::from_mode(0o755)).unwrap();

    assert!(matches!(result, Err(StoreError::Io { .. })), "{result:?}");
    assert_eq!(read(&s.file), "before");
    assert_eq!(fs::read_dir(folder).unwrap().count(), 1);
    // The kept original is still there for a retry, which now succeeds.
    assert_eq!(s.versions.list(&s.file).unwrap().len(), 1);
    backed.write_in_place().unwrap();
    assert_eq!(read(&s.file), "after");
}

#[test]
fn backing_up_a_missing_file_fails_and_keeps_nothing() {
    let s = scratch("x");
    let gone = s.file.with_file_name("gone.txt");
    let result = s.versions.back_up(Pending::new(&gone, vec![]), SavedAt(1));
    assert!(matches!(result, Err(StoreError::Io { .. })), "{result:?}");
    assert_eq!(s.versions.list(&gone).unwrap(), []);
}

#[test]
fn versions_list_newest_first_with_each_originals_content() {
    let s = scratch("v0");
    save(&s, "v1", 300);
    save(&s, "v2", 100 + DAY);
    save(&s, "v3", 200 + DAY);

    let times: Vec<u64> = s
        .versions
        .list(&s.file)
        .unwrap()
        .iter()
        .map(|v| v.saved_at.0)
        .collect();
    assert_eq!(times, [200 + DAY, 100 + DAY, 300]);
    assert_eq!(read(&s.file), "v3");
}

#[test]
fn versions_of_other_files_are_not_listed() {
    let s = scratch("a");
    let other = s.file.with_file_name("other.txt");
    fs::write(&other, "o").unwrap();
    save(&s, "a2", 1);
    assert_eq!(s.versions.list(&other).unwrap(), []);
}

#[test]
fn two_saves_in_the_same_second_do_not_collide() {
    let s = scratch("v0");
    save(&s, "v1", 50);
    save(&s, "v2", 50);

    let versions = s.versions.list(&s.file).unwrap();
    assert_eq!(versions.len(), 2);
    assert_ne!(versions[0].id, versions[1].id);
    // The later save kept v1, the earlier kept v0, and the later one lists first.
    s.versions.restore(&versions[1].id, SavedAt(60)).unwrap();
    assert_eq!(read(&s.file), "v0");
    s.versions.restore(&versions[0].id, SavedAt(61)).unwrap();
    assert_eq!(read(&s.file), "v1");
}

#[test]
fn restore_keeps_the_current_file_first_so_it_can_be_undone() {
    let s = scratch("v0");
    save(&s, "v1", 10);
    let oldest = s.versions.list(&s.file).unwrap()[0].id.clone();

    let written = s.versions.restore(&oldest, SavedAt(20)).unwrap();
    assert_eq!(read(&s.file), "v0");
    let versions = s.versions.list(&s.file).unwrap();
    assert_eq!(versions.len(), 2);
    assert_eq!(versions[0].id, written.kept);
    assert_eq!(versions[0].saved_at, SavedAt(20));

    // Undoing the restore brings back what the file held before it.
    s.versions.restore(&versions[0].id, SavedAt(30)).unwrap();
    assert_eq!(read(&s.file), "v1");
}

#[test]
fn restoring_an_unknown_version_is_an_error() {
    let s = scratch("v0");
    save(&s, "v1", 10);
    let id = s.versions.list(&s.file).unwrap()[0].id.clone();
    s.versions
        .prune(KeepPeriod::days(0), SavedAt(1_000))
        .unwrap();
    let result = s.versions.restore(&id, SavedAt(1_001));
    assert!(
        matches!(result, Err(StoreError::NoSuchVersion { .. })),
        "{result:?}"
    );
    assert_eq!(read(&s.file), "v1");
}

/// (name, keep period, now, how many of the versions kept at day 0, 10, 29 and 31 remain)
const PRUNE_CASES: &[(&str, KeepPeriod, u64, usize)] = &[
    (
        "default keeps the last thirty days",
        DEFAULT_KEEP,
        31 * DAY + 1,
        3,
    ),
    (
        "default at the edge keeps everything",
        DEFAULT_KEEP,
        30 * DAY,
        4,
    ),
    (
        "seven days keeps the last two",
        KeepPeriod::days(7),
        31 * DAY,
        2,
    ),
    (
        "a long period keeps everything",
        KeepPeriod::days(365),
        100 * DAY,
        4,
    ),
];

#[test]
fn prune_removes_only_versions_older_than_the_keep_period() {
    for (name, keep, now, remaining) in PRUNE_CASES {
        let s = scratch("v");
        for day in [0, 10, 29, 31] {
            save(&s, &format!("d{day}"), day * DAY);
        }
        let before = s.versions.list(&s.file).unwrap().len();
        let removed = s.versions.prune(*keep, SavedAt(*now)).unwrap();
        let left = s.versions.list(&s.file).unwrap();

        assert_eq!(before, 4, "{name}");
        assert_eq!(left.len(), *remaining, "{name}");
        assert_eq!(removed, before - remaining, "{name}");
        assert!(
            left.iter().all(|v| v.saved_at.0 + keep.seconds() >= *now),
            "{name}: an old version survived"
        );
    }
}

#[test]
fn prune_deletes_the_bytes_and_empties_the_folder() {
    let s = scratch("v");
    save(&s, "w", 1);
    let root = s
        .file
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("state/anyview/versions");
    assert_eq!(
        s.versions
            .prune(KeepPeriod::days(1), SavedAt(10 * DAY))
            .unwrap(),
        1
    );
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
}

#[test]
fn pruning_a_store_that_does_not_exist_removes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let versions = Versions::new(dir.path().join("never"));
    assert_eq!(versions.prune(DEFAULT_KEEP, SavedAt(1)).unwrap(), 0);
}
