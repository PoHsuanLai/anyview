//! The versions store as a place on disk: damage, junk, size, privacy and what a rename does.

use anyview_core::ByteLen;
use anyview_store::{DEFAULT_KEEP, KeepPeriod, Pending, SavedAt, Versions};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const DAY: u64 = 86_400;

struct Scratch {
    dir: tempfile::TempDir,
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
        dir,
        versions,
        file,
    }
}

impl Scratch {
    fn root(&self) -> PathBuf {
        self.dir.path().join("state/anyview/versions")
    }

    fn folder(&self) -> PathBuf {
        let mut folders: Vec<_> = fs::read_dir(self.root()).unwrap().collect();
        assert_eq!(folders.len(), 1);
        folders.remove(0).unwrap().path()
    }
}

fn save(s: &Scratch, bytes: &str, at: u64) {
    save_file(&s.versions, &s.file, bytes, at);
}

fn save_file(versions: &Versions, file: &Path, bytes: &str, at: u64) {
    versions
        .back_up(Pending::new(file, bytes.into()), SavedAt(at))
        .unwrap()
        .write_in_place()
        .unwrap();
}

fn age(path: &Path, seconds: u64) {
    let when = SystemTime::now() - Duration::from_secs(seconds);
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(when)
        .unwrap();
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[test]
fn one_corrupt_sidecar_does_not_hide_the_other_versions() {
    let s = scratch("v0");
    save(&s, "v1", 100);
    save(&s, "v2", 200);
    save(&s, "v3", 300);
    let mut sidecars: Vec<_> = fs::read_dir(s.folder())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    sidecars.sort();
    fs::write(&sidecars[1], "{ not json").unwrap();

    let listed = s.versions.list(&s.file).unwrap();
    assert_eq!(listed.len(), 2);
    // Pruning leaves the damaged one for a person to look at.
    s.versions.prune(DEFAULT_KEEP, SavedAt(300)).unwrap();
    assert!(sidecars[1].exists());
}

#[test]
fn prune_removes_what_a_crash_left_half_made() {
    let s = scratch("v0");
    save(&s, "v1", now());
    let folder = s.folder();
    let orphan = folder.join("000000000001-0000.bin");
    let tmp = folder.join("000000000001-0000.json.tmp");
    fs::write(&orphan, "x").unwrap();
    fs::write(&tmp, "x").unwrap();
    let young = folder.join("000000000002-0000.bin");
    fs::write(&young, "being written now").unwrap();
    // A folder holding nothing but a stray copy.
    let lone = s.root().join("00000000000000aa");
    fs::create_dir_all(&lone).unwrap();
    fs::write(lone.join("000000000001-0000.bin"), "x").unwrap();
    for path in [&orphan, &tmp, &lone.join("000000000001-0000.bin")] {
        age(path, 3 * DAY);
    }

    let removed = s.versions.prune(DEFAULT_KEEP, SavedAt(now())).unwrap();
    assert_eq!(removed, 0, "no version was old");
    assert!(!orphan.exists());
    assert!(!tmp.exists());
    assert!(!lone.exists(), "the folder of strays went with them");
    assert!(
        young.exists(),
        "a file that may be in the middle of a save stays"
    );
    assert_eq!(s.versions.list(&s.file).unwrap().len(), 1);
}

#[test]
fn a_clock_far_ahead_of_every_version_deletes_nothing() {
    let s = scratch("v0");
    save(&s, "v1", 1_000);
    save(&s, "v2", 1_000 + DAY);
    let jump = SavedAt(1_000 + 20 * 365 * DAY);
    assert_eq!(s.versions.prune(KeepPeriod::days(1), jump).unwrap(), 0);
    assert_eq!(s.versions.list(&s.file).unwrap().len(), 2);
    // A clock a little ahead is believed.
    let later = SavedAt(1_000 + 40 * DAY);
    assert_eq!(s.versions.prune(DEFAULT_KEEP, later).unwrap(), 2);
}

#[test]
fn past_the_size_cap_the_oldest_go_first_and_each_files_newest_stays() {
    let dir = tempfile::tempdir().unwrap();
    let versions = Versions::under_state(&dir.path().join("state")).with_cap(ByteLen(303));
    let a = dir.path().join("a.txt");
    let b = dir.path().join("b.txt");
    fs::write(&a, "a0").unwrap();
    fs::write(&b, "b0").unwrap();
    let big = "x".repeat(100);
    // a keeps three versions (the first two are 2 and 100 bytes), b one of 100 bytes.
    save_file(&versions, &a, &big, 10);
    save_file(&versions, &a, &format!("{big}1"), 20);
    save_file(&versions, &a, &format!("{big}22"), 30);
    save_file(&versions, &b, &big, 15);
    save_file(&versions, &b, "b1", 25);

    versions.prune(KeepPeriod::days(365), SavedAt(40)).unwrap();
    let kept = |file: &Path| -> Vec<u64> {
        versions
            .list(file)
            .unwrap()
            .iter()
            .map(|v| v.saved_at.0)
            .collect()
    };
    // Versions kept: a at 10 (2 bytes), 20 (100), 30 (101); b at 15 (2), 25 (100): 305 bytes. The
    // oldest, a's at 10, goes, and the rest fit.
    assert_eq!(kept(&a), [30, 20]);
    assert_eq!(kept(&b), [25, 15]);

    let tiny = Versions::under_state(&dir.path().join("state")).with_cap(ByteLen(0));
    tiny.prune(KeepPeriod::days(365), SavedAt(40)).unwrap();
    assert_eq!(kept(&a), [30], "a file's newest version is never dropped");
    assert_eq!(kept(&b), [25], "a file's newest version is never dropped");
}

#[test]
fn kept_versions_and_their_folders_are_private() {
    let s = scratch("v0");
    fs::set_permissions(&s.file, fs::Permissions::from_mode(0o644)).unwrap();
    save(&s, "v1", 1);
    let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&s.root()), 0o700);
    assert_eq!(mode(&s.folder()), 0o700);
    for entry in fs::read_dir(s.folder()).unwrap() {
        assert_eq!(mode(&entry.unwrap().path()), 0o600);
    }
}

#[test]
fn a_renamed_files_versions_follow_it() {
    let s = scratch("v0");
    save(&s, "v1", 5);
    save(&s, "v2", 6);
    let from = fs::canonicalize(&s.file).unwrap();
    let to = from.with_file_name("renamed.txt");
    fs::rename(&from, &to).unwrap();

    assert_eq!(s.versions.rekey(&from, &to).unwrap(), 2);
    assert_eq!(s.versions.list(&from).unwrap(), []);
    let moved = s.versions.list(&to).unwrap();
    assert_eq!(moved.len(), 2);
    assert!(moved.iter().all(|v| v.path == to));
    s.versions.restore(&moved[1].id, SavedAt(9)).unwrap();
    assert_eq!(fs::read_to_string(&to).unwrap(), "v0");
}
