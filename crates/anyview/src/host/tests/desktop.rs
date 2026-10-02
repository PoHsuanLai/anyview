use super::support::{NOW, PDF, PNG, desktop, entry, path, probed};
use crate::host::{Hosting, Outcome, Task};
use anyview_core::{FileName, FormatKind, Resume};
use anyview_platform::{DesktopId, ShareTarget};
use anyview_store::{HistoryRead, read_history};

#[tokio::test]
async fn open_with_skips_the_viewer_itself_and_opens_in_the_next_program() {
    let dir = tempfile::tempdir().unwrap();
    let image = probed(dir.path(), "a.png", PNG);
    let (desktop, fakes) = desktop(
        dir.path(),
        vec![
            entry("org.quire.Anyview.desktop"),
            entry("org.gnome.eog.desktop"),
        ],
    );
    let outcome = desktop
        .carry_out(Task::OpenWith(image.clone()))
        .await
        .unwrap();
    assert_eq!(outcome, Outcome::Done);
    assert_eq!(
        fakes.apps.opened(),
        vec![(
            DesktopId::new("org.gnome.eog.desktop").unwrap(),
            image.source.path().clone()
        )]
    );
}

#[tokio::test]
async fn open_with_has_nothing_to_do_when_only_the_viewer_handles_the_type() {
    let dir = tempfile::tempdir().unwrap();
    let image = probed(dir.path(), "a.png", PNG);
    let (desktop, fakes) = desktop(dir.path(), vec![entry("org.quire.Anyview.desktop")]);
    let outcome = desktop.carry_out(Task::OpenWith(image)).await.unwrap();
    assert!(matches!(outcome, Outcome::Nothing(_)), "{outcome:?}");
    assert!(fakes.apps.opened().is_empty());
}

#[tokio::test]
async fn reveal_share_and_trash_reach_their_platform_trait_with_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = path("/tmp/somewhere/a.png");
    let (desktop, fakes) = desktop(dir.path(), vec![]);
    for task in [
        Task::Reveal(file.clone()),
        Task::Share(file.clone()),
        Task::Trash(file.clone()),
    ] {
        assert_eq!(desktop.carry_out(task).await.unwrap(), Outcome::Done);
    }
    assert_eq!(fakes.reveal.revealed(), vec![file.clone()]);
    assert_eq!(
        fakes.share.shared(),
        vec![(file.clone(), ShareTarget::Mail)]
    );
    assert_eq!(fakes.trash.trashed(), vec![file]);
}

#[tokio::test]
async fn print_hands_the_pdf_bytes_and_its_name_to_the_printer() {
    let dir = tempfile::tempdir().unwrap();
    let pdf = probed(dir.path(), "report.pdf", PDF);
    let (desktop, fakes) = desktop(dir.path(), vec![]);
    assert_eq!(
        desktop.carry_out(Task::Print(pdf)).await.unwrap(),
        Outcome::Done
    );
    let jobs = fakes.printer.jobs();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].0.0, "report.pdf");
    assert_eq!(jobs[0].1, PDF.len());
}

#[tokio::test]
async fn a_view_is_recorded_in_the_history_and_the_place_is_kept_and_found_again() {
    let dir = tempfile::tempdir().unwrap();
    let image = probed(dir.path(), "a.png", PNG);
    let (desktop, _) = desktop(dir.path(), vec![]);
    desktop
        .carry_out(Task::RecordView(image.clone()))
        .await
        .unwrap();
    let HistoryRead::Loaded(history) = read_history(&dir.path().join("store")) else {
        panic!("the history was not written");
    };
    let entries = &history.entries;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].path, *image.source.path());
    assert_eq!(entries[0].kind, FormatKind::Raster);
    assert_eq!(entries[0].viewed, NOW);

    assert_eq!(desktop.resume(&image.source), None, "nothing kept yet");
    let kept = Resume::Text {
        line: anyview_core::LineIndex(41),
    };
    let outcome = desktop
        .carry_out(Task::Remember {
            source: image.source.clone(),
            resume: kept.clone(),
        })
        .await
        .unwrap();
    assert_eq!(outcome, Outcome::Done);
    assert_eq!(desktop.resume(&image.source), Some(kept));
}

#[tokio::test]
async fn rename_moves_the_file_and_refuses_to_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let image = probed(dir.path(), "a.png", PNG);
    std::fs::write(dir.path().join("taken.png"), b"x").unwrap();
    let (desktop, _) = desktop(dir.path(), vec![]);
    let file = image.source.path().clone();

    let refused = desktop
        .carry_out(Task::Rename {
            file: file.clone(),
            to: FileName::new("taken.png").unwrap(),
        })
        .await
        .unwrap();
    assert!(matches!(refused, Outcome::Failed(_)), "{refused:?}");
    assert!(file.as_path().exists(), "the file stayed");

    let moved = desktop
        .carry_out(Task::Rename {
            file: file.clone(),
            to: FileName::new("b.png").unwrap(),
        })
        .await
        .unwrap();
    assert_eq!(
        moved,
        Outcome::Moved(path(dir.path().join("b.png").to_str().unwrap()))
    );
    assert!(!file.as_path().exists());
    assert!(dir.path().join("b.png").exists());
}

#[tokio::test]
async fn duplicate_writes_the_first_free_copy_name() {
    let dir = tempfile::tempdir().unwrap();
    let image = probed(dir.path(), "a.png", PNG);
    let (desktop, _) = desktop(dir.path(), vec![]);
    for _ in 0..2 {
        let outcome = desktop
            .carry_out(Task::Duplicate(image.source.path().clone()))
            .await
            .unwrap();
        assert_eq!(outcome, Outcome::Done);
    }
    assert_eq!(std::fs::read(dir.path().join("a copy.png")).unwrap(), PNG);
    assert_eq!(std::fs::read(dir.path().join("a copy 2.png")).unwrap(), PNG);
}
