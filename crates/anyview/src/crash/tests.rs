use super::*;
use std::path::Path;

fn report(at: u64) -> Report {
    Report {
        thread: "main".to_owned(),
        message: "boom".to_owned(),
        location: "src/x.rs:1:2".to_owned(),
        version: "0.1.0-beta.1".to_owned(),
        at,
    }
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn a_report_names_thread_message_location_and_version() {
    let text = report(5).to_string();
    for part in ["0.1.0-beta.1", "main", "boom", "src/x.rs:1:2"] {
        assert!(text.contains(part), "{part} in {text}");
    }
}

#[test]
fn the_folder_is_under_the_state_directory() {
    assert_eq!(
        crash_dir(Path::new("/s")),
        Path::new("/s/anyview/crash").to_path_buf()
    );
}

#[test]
fn a_report_lands_in_the_scratch_state_folder() {
    let scratch = tempfile::tempdir().unwrap();
    let dir = crash_dir(scratch.path());
    let path = write_report(&dir, &report(1_700_000_000)).unwrap();
    assert!(path.starts_with(scratch.path()));
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        report(1_700_000_000).to_string()
    );
}

#[test]
fn only_the_newest_ten_are_kept_and_one_second_can_hold_several() {
    let scratch = tempfile::tempdir().unwrap();
    let dir = crash_dir(scratch.path());
    for at in 1..=15 {
        write_report(&dir, &report(at)).unwrap();
    }
    write_report(&dir, &report(15)).unwrap();
    let kept = names(&dir);
    assert_eq!(kept.len(), KEEP);
    assert_eq!(
        kept.first().map(String::as_str),
        Some("000000000007-000.txt")
    );
    assert_eq!(
        kept.last().map(String::as_str),
        Some("000000000015-001.txt")
    );
}

#[test]
fn pruning_leaves_files_that_are_not_reports() {
    let scratch = tempfile::tempdir().unwrap();
    let dir = crash_dir(scratch.path());
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("notes.md"), "mine").unwrap();
    for at in 1..=12 {
        write_report(&dir, &report(at)).unwrap();
    }
    assert!(dir.join("notes.md").exists());
    assert_eq!(names(&dir).len(), KEEP + 1);
}
