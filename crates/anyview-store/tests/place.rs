//! Claiming names beside a file: free names, no-replace renames and copies, in scratch folders.

#![allow(clippy::unwrap_used)]

use anyview_store::{StoreError, copy_new, free_beside, is_free, is_taken, rename_noreplace};
use std::fs;
use std::path::Path;

fn hidden(dir: &Path) -> usize {
    fs::read_dir(dir)
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with('.')
        })
        .count()
}

#[test]
fn a_name_beside_the_file_is_the_first_free_one() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("Holiday.mkv");
    let name = |text: &str| dir.path().join(text);
    assert_eq!(
        free_beside(&file, " frame", "png"),
        Some(name("Holiday frame.png"))
    );
    fs::write(name("Holiday frame.png"), "x").unwrap();
    assert_eq!(
        free_beside(&file, " frame", "png"),
        Some(name("Holiday frame 2.png")),
        "a taken name is never overwritten"
    );
    // A link that points nowhere holds its name too: a file made there would be made at its target.
    std::os::unix::fs::symlink(name("nowhere"), name("Holiday frame 2.png")).unwrap();
    assert!(!is_free(&name("Holiday frame 2.png")));
    assert_eq!(
        free_beside(&file, " frame", "png"),
        Some(name("Holiday frame 3.png"))
    );
    assert_eq!(
        free_beside(&file, " frame", "jpg"),
        Some(name("Holiday frame.jpg")),
        "another extension is another name"
    );
    assert_eq!(
        free_beside(&name("README"), " copy", ""),
        Some(name("README copy")),
        "no extension, no dot"
    );
}

#[test]
fn a_rename_never_replaces_a_file_or_a_dangling_link() {
    let dir = tempfile::tempdir().unwrap();
    let (from, taken, link) = (
        dir.path().join("a.txt"),
        dir.path().join("b.txt"),
        dir.path().join("c.txt"),
    );
    fs::write(&from, "a").unwrap();
    fs::write(&taken, "b").unwrap();
    std::os::unix::fs::symlink(dir.path().join("nowhere"), &link).unwrap();

    for to in [&taken, &link] {
        let error = rename_noreplace(&from, to).unwrap_err();
        assert!(is_taken(&error), "{error:?}");
    }
    assert_eq!(fs::read_to_string(&from).unwrap(), "a");
    assert_eq!(fs::read_to_string(&taken).unwrap(), "b");

    let free = dir.path().join("d.txt");
    rename_noreplace(&from, &free).unwrap();
    assert_eq!(fs::read_to_string(&free).unwrap(), "a");
    assert!(!from.exists());
}

#[test]
fn a_folder_is_renamed_the_same_way() {
    let dir = tempfile::tempdir().unwrap();
    let (from, taken) = (dir.path().join("one"), dir.path().join("two"));
    fs::create_dir(&from).unwrap();
    fs::create_dir(&taken).unwrap();
    assert!(is_taken(&rename_noreplace(&from, &taken).unwrap_err()));
    rename_noreplace(&from, &dir.path().join("three")).unwrap();
}

#[test]
fn a_copy_is_whole_or_absent_and_never_replaces() {
    let dir = tempfile::tempdir().unwrap();
    let (from, to) = (dir.path().join("a.bin"), dir.path().join("a copy.bin"));
    fs::write(&from, vec![7_u8; 300_000]).unwrap();
    copy_new(&from, &to).unwrap();
    assert_eq!(fs::read(&to).unwrap(), vec![7_u8; 300_000]);
    assert_eq!(hidden(dir.path()), 0, "no temporary file is left");

    fs::write(&to, "mine").unwrap();
    let error = copy_new(&from, &to).unwrap_err();
    assert!(is_taken(&error), "{error:?}");
    assert_eq!(fs::read_to_string(&to).unwrap(), "mine");
    assert_eq!(hidden(dir.path()), 0);

    let elsewhere = dir.path().join("elsewhere");
    let link = dir.path().join("link.bin");
    std::os::unix::fs::symlink(&elsewhere, &link).unwrap();
    assert!(is_taken(&copy_new(&from, &link).unwrap_err()));
    assert!(!elsewhere.exists(), "the link's target was not created");

    let missing = copy_new(&dir.path().join("gone"), &dir.path().join("x"));
    assert!(matches!(missing, Err(StoreError::Io { .. })));
    assert_eq!(hidden(dir.path()), 0);
}
