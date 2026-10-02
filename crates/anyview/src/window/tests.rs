use super::*;
use anyview_core::FilePath;

fn touch(dir: &std::path::Path, name: &str) -> FilePath {
    std::fs::write(dir.join(name), "x").unwrap();
    FilePath::new(dir.join(name)).unwrap()
}

#[test]
fn the_sequence_is_the_visible_files_of_the_folder_by_name_pointing_at_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let b = touch(dir.path(), "b.png");
    let a = touch(dir.path(), "a.png");
    let c = touch(dir.path(), "c.txt");
    touch(dir.path(), ".hidden");
    std::fs::create_dir(dir.path().join("folder")).unwrap();

    let sequence = sequence_around(&b).unwrap();
    let entries: Vec<&FilePath> = sequence.entries().iter().collect();
    assert_eq!(entries, vec![&a, &b, &c], "no hidden file and no folder");
    assert_eq!(sequence.current(), &b);
}

#[test]
fn a_file_that_is_not_there_or_a_folder_that_cannot_be_read_has_no_sequence() {
    let dir = tempfile::tempdir().unwrap();
    let missing = FilePath::new(dir.path().join("gone.png")).unwrap();
    assert_eq!(sequence_around(&missing), None, "not among the files");
    let nowhere = FilePath::new(dir.path().join("no-such-folder/a.png")).unwrap();
    assert_eq!(sequence_around(&nowhere), None, "no folder");
    assert_eq!(
        Opening::around(missing.clone()),
        Opening {
            file: missing,
            sequence: None
        },
        "the window still opens, to say the file is not there"
    );
}
