//! Exports that cannot be written, or would land on a file that is there.

use super::support::{desktop, path, probed};
use crate::host::{Hosting, Outcome, Task};
use anyview_core::{ExportChoice, TextExport, TextExportKind};
use anyview_export::DocumentExport;
use std::os::unix::fs::PermissionsExt;

fn to_pdf() -> DocumentExport {
    DocumentExport::Text(TextExport::default_for(TextExportKind::Pdf))
}

fn names(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn an_export_into_a_folder_that_cannot_be_written_fails_and_leaves_no_trace() {
    let scratch = tempfile::tempdir().unwrap();
    let locked = tempfile::tempdir().unwrap();
    let notes = probed(locked.path(), "notes.md", b"# Minutes\n");
    let (desktop, _) = desktop(scratch.path(), vec![]);
    std::fs::set_permissions(locked.path(), std::fs::Permissions::from_mode(0o555)).unwrap();
    let before = names(locked.path());
    let outcome = desktop
        .carry_out(Task::ExportDocument {
            file: notes,
            choice: to_pdf(),
        })
        .await
        .unwrap();
    let after = names(locked.path());
    std::fs::set_permissions(locked.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(outcome, Outcome::Failed(_)), "{outcome:?}");
    assert_eq!(after, before, "no half-written file is left");
}

#[tokio::test]
async fn exporting_twice_never_overwrites_the_first_export() {
    let dir = tempfile::tempdir().unwrap();
    let notes = probed(dir.path(), "notes.md", b"# Minutes\n");
    std::fs::write(dir.path().join("notes.pdf"), b"precious").unwrap();
    let (desktop, _) = desktop(dir.path(), vec![]);
    let outcome = desktop
        .carry_out(Task::ExportDocument {
            file: notes,
            choice: to_pdf(),
        })
        .await
        .unwrap();
    assert_eq!(
        std::fs::read(dir.path().join("notes.pdf")).unwrap(),
        b"precious"
    );
    assert!(
        matches!(outcome, Outcome::Wrote(ref made) if *made != path(dir.path().join("notes.pdf").to_str().unwrap())),
        "{outcome:?}"
    );
}

#[tokio::test]
async fn an_export_of_a_file_deleted_meanwhile_fails() {
    let dir = tempfile::tempdir().unwrap();
    let notes = probed(dir.path(), "notes.md", b"# Minutes\n");
    std::fs::remove_file(dir.path().join("notes.md")).unwrap();
    let (desktop, _) = desktop(dir.path(), vec![]);
    let outcome = desktop
        .carry_out(Task::ExportDocument {
            file: notes,
            choice: to_pdf(),
        })
        .await
        .unwrap();
    assert!(matches!(outcome, Outcome::Failed(_)), "{outcome:?}");
    assert_eq!(names(dir.path()), Vec::<String>::new());
}
