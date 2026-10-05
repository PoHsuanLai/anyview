//! What a save keeps and refuses, through the public API in scratch directories: the file's mode,
//! owner, extended attributes and hard links, a read-only file, a failing folder sync, an outside
//! change in the middle of a save, and saves that race.

#![allow(clippy::unwrap_used)]

use anyview_store::{Durability, Pending, SavedAt, StoreError, StoreOp, Versions, is_read_only};
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::PathBuf;
use std::sync::Arc;

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

fn save(s: &Scratch, bytes: &str, at: u64) -> Result<anyview_store::Written, StoreError> {
    s.versions
        .back_up(Pending::new(&s.file, bytes.into()), SavedAt(at))?
        .write_in_place()
}

fn names(dir: &std::path::Path) -> Vec<String> {
    let mut found: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    found.sort();
    found
}

#[test]
fn a_failing_folder_sync_after_the_rename_is_still_a_saved_file() {
    let s = scratch("before");
    let folder = s.file.parent().unwrap();
    // Write and search, no read: the rename works and opening the folder to sync it does not.
    fs::set_permissions(folder, fs::Permissions::from_mode(0o300)).unwrap();
    let result = save(&s, "after", 1);
    fs::set_permissions(folder, fs::Permissions::from_mode(0o755)).unwrap();

    let written = result.unwrap();
    assert_eq!(written.durability, Durability::Unconfirmed);
    assert_eq!(fs::read_to_string(&s.file).unwrap(), "after");
}

#[test]
fn a_save_keeps_special_mode_bits_and_user_attributes() {
    let s = scratch("before");
    fs::set_permissions(&s.file, fs::Permissions::from_mode(0o2750)).unwrap();
    let marked = rustix::fs::setxattr(
        &s.file,
        "user.anyview.test",
        b"tagged",
        rustix::fs::XattrFlags::empty(),
    );
    save(&s, "after", 1).unwrap();

    let mode = fs::metadata(&s.file).unwrap().permissions().mode() & 0o7777;
    assert_eq!(mode, 0o2750, "set-group-id and the permission bits");
    if marked.is_ok() {
        let mut value = [0_u8; 16];
        let length = rustix::fs::getxattr(&s.file, "user.anyview.test", &mut value[..]).unwrap();
        assert_eq!(&value[..length], b"tagged");
    }
}

#[test]
fn a_file_with_another_hard_link_is_written_in_place_so_the_links_stay_one_file() {
    let s = scratch("before");
    let link = s.file.with_file_name("linked.txt");
    fs::hard_link(&s.file, &link).unwrap();
    let inode = fs::metadata(&s.file).unwrap().ino();

    save(&s, "after, longer than before", 1).unwrap();
    assert_eq!(
        fs::read_to_string(&link).unwrap(),
        "after, longer than before"
    );
    assert_eq!(fs::metadata(&s.file).unwrap().ino(), inode);
    assert_eq!(fs::metadata(&s.file).unwrap().nlink(), 2);
    save(&s, "short", 2).unwrap();
    assert_eq!(
        fs::read_to_string(&link).unwrap(),
        "short",
        "and it shrinks"
    );
    assert_eq!(s.versions.list(&s.file).unwrap().len(), 2);
}

#[test]
fn a_read_only_file_is_refused_and_keeps_nothing() {
    let s = scratch("before");
    fs::set_permissions(&s.file, fs::Permissions::from_mode(0o444)).unwrap();
    assert!(is_read_only(&s.file));

    let result = save(&s, "after", 1);
    assert!(
        matches!(
            result,
            Err(StoreError::Io {
                op: StoreOp::Permissions,
                kind: std::io::ErrorKind::PermissionDenied,
                ..
            })
        ),
        "{result:?}"
    );
    assert_eq!(fs::read_to_string(&s.file).unwrap(), "before");
    assert_eq!(s.versions.list(&s.file).unwrap(), []);

    fs::set_permissions(&s.file, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(!is_read_only(&s.file));
    save(&s, "after", 2).unwrap();
}

#[test]
fn a_change_made_between_the_backup_and_the_rename_is_kept_not_lost() {
    let s = scratch("before");
    let backed = s
        .versions
        .back_up(Pending::new(&s.file, b"mine".to_vec()), SavedAt(10))
        .unwrap();
    // Someone else writes the file after the backup.
    fs::write(&s.file, "theirs, a different length").unwrap();
    backed.write_in_place().unwrap();

    assert_eq!(fs::read_to_string(&s.file).unwrap(), "mine");
    assert_eq!(
        s.versions.list(&s.file).unwrap().len(),
        2,
        "the original and the outside change"
    );
    // Restoring the newest brings back the outside change.
    let newest = s.versions.list(&s.file).unwrap().remove(0);
    s.versions.restore(&newest.id, SavedAt(11)).unwrap();
    assert_eq!(
        fs::read_to_string(&s.file).unwrap(),
        "theirs, a different length"
    );
}

#[test]
fn a_save_that_fails_before_touching_the_file_keeps_no_copy_of_it() {
    let s = scratch("before");
    let backed = s
        .versions
        .back_up(Pending::new(&s.file, b"after".to_vec()), SavedAt(1))
        .unwrap();
    let folder = s.file.parent().unwrap();
    fs::set_permissions(folder, fs::Permissions::from_mode(0o555)).unwrap();
    let failed = backed.write_in_place();
    fs::set_permissions(folder, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(failed.is_err());
    assert_eq!(s.versions.list(&s.file).unwrap(), []);
    let root = s
        .file
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("state/anyview/versions");
    assert_eq!(
        names(&root),
        Vec::<String>::new(),
        "and its folder went too"
    );
}

#[test]
fn a_version_identical_to_the_latest_is_not_made_again() {
    let s = scratch("same");
    // Saves of identical bytes over identical bytes: one kept copy serves them all.
    save(&s, "same", 1).unwrap();
    save(&s, "same", 2).unwrap();
    save(&s, "same", 3).unwrap();
    assert_eq!(s.versions.list(&s.file).unwrap().len(), 1);
}

#[test]
fn saves_of_one_file_from_many_threads_each_land_whole_and_leave_no_temporary_file() {
    let s = Arc::new(scratch("start"));
    let handles: Vec<_> = (0..8_u64)
        .map(|n| {
            let s = Arc::clone(&s);
            std::thread::spawn(move || {
                let bytes = format!("{n}").repeat(200_000 + n as usize);
                save(&s, &bytes, 100 + n).unwrap();
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
    let text = fs::read_to_string(&s.file).unwrap();
    let digit = text.chars().next().unwrap();
    assert!(text.chars().all(|c| c == digit), "a mixture of two saves");
    assert_eq!(names(s.file.parent().unwrap()), ["doc.txt"]);
}

#[test]
fn a_temporary_file_of_a_dead_process_is_removed_by_the_next_save() {
    let s = scratch("before");
    let folder = s.file.parent().unwrap();
    // 4194304 is above the kernel's largest pid, so no process has it.
    let dead = folder.join(".doc.txt.anyview-4194304-3.tmp");
    let live = folder.join(format!(".other.anyview-{}-3.tmp", std::process::id()));
    let mine = folder.join(".notes.tmp");
    for file in [&dead, &live, &mine] {
        fs::write(file, "x").unwrap();
    }
    save(&s, "after", 1).unwrap();
    assert!(!dead.exists());
    assert!(live.exists(), "a running process's file stays");
    assert!(mine.exists(), "a person's file stays");
}
