//! A panic on another thread writes its report into the state folder it was told, and into no
//! other: the folder is a scratch one.
//!
//! Its own binary, run in a process of its own: it installs the panic hook, which is process-global, so a panic on another test's thread would write a report into its scratch folder.

use anyview::crash::{crash_dir, install};

#[test]
fn a_panic_on_a_thread_leaves_a_report_in_the_scratch_state_folder() {
    let scratch = tempfile::tempdir().unwrap();
    let dir = crash_dir(scratch.path());
    install(dir.clone(), "9.9.9-test", || 1_700_000_000);

    let outcome = std::thread::Builder::new()
        .name("view-job".to_owned())
        .spawn(|| panic!("the decoder fell over"))
        .unwrap()
        .join();
    drop(std::panic::take_hook());
    assert!(outcome.is_err());

    let files: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
    assert_eq!(files.len(), 1);
    let text = std::fs::read_to_string(files[0].as_ref().unwrap().path()).unwrap();
    for part in [
        "9.9.9-test",
        "view-job",
        "the decoder fell over",
        "crash_report.rs",
    ] {
        assert!(text.contains(part), "{part} in {text}");
    }
}
