//! Rename through the host: what a typed name does to the file, for the names a person might type.

use super::support::{PNG, desktop, probed};
use crate::host::{Carry, Hosting, Outcome, Shown, Task, route};
use anyview_core::FileName;
use anyview_ui::{HostRequest, TypedText};

fn renamed_to(typed: &str) -> (tempfile::TempDir, Carry) {
    let dir = tempfile::tempdir().unwrap();
    let file = probed(dir.path(), "a.png", PNG);
    let (shown, _) = route(Shown::default(), HostRequest::Opened(file));
    let (_, carry) = route(shown, HostRequest::Rename(TypedText::new(typed)));
    (dir, carry)
}

#[tokio::test]
async fn renaming_a_file_to_its_own_name_is_not_a_name_clash() {
    let dir = tempfile::tempdir().unwrap();
    let file = probed(dir.path(), "a.png", PNG);
    let (desktop, _) = desktop(dir.path());
    let outcome = desktop
        .carry_out(Task::Rename {
            file: file.source.path().clone(),
            to: FileName::new("a.png").unwrap(),
        })
        .await
        .unwrap();
    assert_ne!(
        outcome,
        Outcome::Taken,
        "the person left the name as it was; \"A file with that name already exists\" is wrong"
    );
    assert!(dir.path().join("a.png").exists());
}

#[tokio::test]
async fn a_name_of_only_spaces_is_not_a_file_name() {
    for typed in ["   ", "\t", " "] {
        let (dir, carry) = renamed_to(typed);
        assert!(
            matches!(carry, Carry::Declined(_)),
            "{typed:?} is refused, not carried out as a rename to a name nobody can see: {carry:?}"
        );
        assert!(dir.path().join("a.png").exists());
    }
}

#[tokio::test]
async fn a_very_long_name_fails_without_losing_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = probed(dir.path(), "a.png", PNG);
    let (desktop, _) = desktop(dir.path());
    let outcome = desktop
        .carry_out(Task::Rename {
            file: file.source.path().clone(),
            to: FileName::new(&"x".repeat(400)).unwrap(),
        })
        .await
        .unwrap();
    assert!(matches!(outcome, Outcome::Failed(_)), "{outcome:?}");
    assert!(dir.path().join("a.png").exists());
}

#[tokio::test]
async fn an_emoji_name_and_an_existing_name() {
    let dir = tempfile::tempdir().unwrap();
    let file = probed(dir.path(), "a.png", PNG);
    std::fs::write(dir.path().join("b.png"), PNG).unwrap();
    let (desktop, _) = desktop(dir.path());
    let taken = desktop
        .carry_out(Task::Rename {
            file: file.source.path().clone(),
            to: FileName::new("b.png").unwrap(),
        })
        .await
        .unwrap();
    assert_eq!(taken, Outcome::Taken);
    let moved = desktop
        .carry_out(Task::Rename {
            file: file.source.path().clone(),
            to: FileName::new("🌅 sunrise.png").unwrap(),
        })
        .await
        .unwrap();
    assert!(matches!(moved, Outcome::Moved(_)), "{moved:?}");
    assert!(dir.path().join("🌅 sunrise.png").exists());
}

#[test]
fn an_empty_name_and_a_name_with_a_slash_are_declined_before_any_task() {
    for typed in ["", "a/b.png", "..", "."] {
        let (_dir, carry) = renamed_to(typed);
        assert!(matches!(carry, Carry::Declined(_)), "{typed:?}: {carry:?}");
    }
}
