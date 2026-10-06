use super::support::{NOW, PDF, PNG, desktop, desktop_choosing, entry, path, probed};
use crate::host::{Hosting, Outcome, Task};
use anyview_core::{
    ExportChoice, FileName, FormatKind, PageSelection, PixelLen, PixelSize, RasterExport,
    RasterTarget, Resize, Resume, TextExport, TextExportKind, TextFlavour,
};
use anyview_export::DocumentExport;
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
async fn print_lays_out_a_picture_and_a_document_as_a_pdf_for_the_printer() {
    let dir = tempfile::tempdir().unwrap();
    let image = probed(dir.path(), "a.png", &png());
    let notes = probed(dir.path(), "notes.md", b"# Minutes\n\nWe agreed to ship.\n");
    let (desktop, fakes) = desktop(dir.path(), vec![]);
    for file in [&image, &notes] {
        let outcome = desktop.carry_out(Task::Print(file.clone())).await.unwrap();
        assert_eq!(outcome, Outcome::Done);
    }
    let titles: Vec<String> = fakes
        .printer
        .jobs()
        .into_iter()
        .map(|job| job.0.0)
        .collect();
    assert_eq!(titles, ["a.png", "notes.md"]);
    let pdfs = fakes.printer.pdfs();
    assert_eq!(pdfs.len(), 2);
    assert!(
        pdfs.iter().all(|pdf| pdf.starts_with(b"%PDF-")),
        "both are PDFs"
    );
    assert_ne!(pdfs[0], std::fs::read(dir.path().join("a.png")).unwrap());
    let text = anyview_pdf_text(&pdfs[1]);
    assert!(
        text.contains("Minutes") && text.contains("We agreed"),
        "{text:?}"
    );
    let listed: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".pdf"))
        .collect();
    assert!(listed.is_empty(), "printing writes no file: {listed:?}");
}

#[tokio::test]
async fn print_of_a_file_that_cannot_be_laid_out_is_a_failure_and_prints_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let broken = probed(dir.path(), "a.png", PNG);
    let (desktop, fakes) = desktop(dir.path(), vec![]);
    let outcome = desktop.carry_out(Task::Print(broken)).await.unwrap();
    assert!(matches!(outcome, Outcome::Failed(_)), "{outcome:?}");
    assert!(fakes.printer.jobs().is_empty());
}

#[tokio::test]
async fn an_export_is_written_beside_the_file_and_a_failed_one_leaves_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let notes = probed(dir.path(), "notes.md", b"# Minutes\n\nWe agreed to ship.\n");
    let broken = probed(dir.path(), "broken.png", PNG);
    let (desktop, _) = desktop(dir.path(), vec![]);
    let to_pdf = DocumentExport::Text(TextExport::default_for(TextExportKind::Pdf));
    let outcome = desktop
        .carry_out(Task::ExportDocument {
            file: notes,
            choice: to_pdf,
        })
        .await
        .unwrap();
    assert_eq!(
        outcome,
        Outcome::Wrote(path(dir.path().join("notes.pdf").to_str().unwrap()))
    );
    let written = std::fs::read(dir.path().join("notes.pdf")).unwrap();
    assert!(anyview_pdf_text(&written).contains("We agreed to ship"));

    let again = DocumentExport::Raster(RasterExport::Image(RasterTarget::Tiff, Resize::Original));
    let outcome = desktop
        .carry_out(Task::ExportDocument {
            file: broken,
            choice: again,
        })
        .await
        .unwrap();
    assert!(matches!(outcome, Outcome::Failed(_)), "{outcome:?}");
    assert!(!dir.path().join("broken.tiff").exists());
}

/// A PNG that decodes: 8 by 8, one colour.
fn png() -> Vec<u8> {
    let size = PixelSize {
        width: PixelLen(8),
        height: PixelLen(8),
    };
    let picture = anyview_image::Rgba8::new(size, vec![90; 8 * 8 * 4]).unwrap();
    anyview_image::encode(&picture, RasterTarget::Png).unwrap()
}

/// The text of a PDF, read the way the viewer reads one.
fn anyview_pdf_text(bytes: &[u8]) -> String {
    let doc = anyview_pdf::PdfDocument::from_bytes(bytes.to_vec()).unwrap();
    anyview_pdf::write_text(&doc, PageSelection::All, TextFlavour::Plain).unwrap()
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
    assert_eq!(
        desktop.resume(&image.source),
        None,
        "the place waits to be written with the ones that follow it"
    );
    desktop.flush();
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
    assert_eq!(refused, Outcome::Taken);
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
    for copy in ["a copy.png", "a copy 2.png"] {
        let outcome = desktop
            .carry_out(Task::Duplicate(image.source.path().clone()))
            .await
            .unwrap();
        assert_eq!(
            outcome,
            Outcome::Wrote(path(dir.path().join(copy).to_str().unwrap()))
        );
    }
    assert_eq!(std::fs::read(dir.path().join("a copy.png")).unwrap(), PNG);
    assert_eq!(std::fs::read(dir.path().join("a copy 2.png")).unwrap(), PNG);
}

#[tokio::test]
async fn the_file_dialog_answers_what_was_chosen_and_nothing_when_it_was_closed() {
    let dir = tempfile::tempdir().unwrap();
    let (desktop, fakes) = desktop(dir.path(), vec![]);
    // The fake dialog is closed: no file, and nothing to tell.
    assert_eq!(
        desktop.carry_out(Task::PickFile).await.unwrap(),
        Outcome::Done
    );
    assert_eq!(fakes.picker.asked(), 1);

    let chosen = vec![path("/tmp/somewhere/a.png"), path("/tmp/somewhere/b.png")];
    let (desktop, _) = desktop_choosing(
        dir.path(),
        anyview_platform::PickOutcome::Chosen(chosen.clone()),
    );
    assert_eq!(
        desktop.carry_out(Task::PickFile).await.unwrap(),
        Outcome::Picked(chosen)
    );

    let (desktop, _) = desktop_choosing(dir.path(), anyview_platform::PickOutcome::NoDialog);
    assert!(matches!(
        desktop.carry_out(Task::PickFile).await.unwrap(),
        Outcome::Nothing(_)
    ));
}

#[tokio::test]
async fn a_link_is_handed_to_the_desktops_handler() {
    let dir = tempfile::tempdir().unwrap();
    let (desktop, fakes) = desktop(dir.path(), vec![]);
    let outcome = desktop
        .carry_out(Task::OpenLink("https://example.org/a".to_owned()))
        .await
        .unwrap();
    assert_eq!(outcome, Outcome::Done);
    assert_eq!(
        fakes.links.opened(),
        vec!["https://example.org/a".to_owned()]
    );
}

#[tokio::test]
async fn rename_never_replaces_a_dangling_symlink_and_its_versions_follow() {
    let dir = tempfile::tempdir().unwrap();
    let image = probed(dir.path(), "a.png", PNG);
    let file = image.source.path().clone();
    std::os::unix::fs::symlink(dir.path().join("nowhere"), dir.path().join("link.png")).unwrap();
    let versions = anyview_store::Versions::under_state(&dir.path().join("state"));
    versions
        .back_up(
            anyview_store::Pending::new(file.as_path(), b"edited".to_vec()),
            anyview_store::SavedAt(7),
        )
        .unwrap()
        .write_in_place()
        .unwrap();
    let (desktop, _) = desktop(dir.path(), vec![]);

    let refused = desktop
        .carry_out(Task::Rename {
            file: file.clone(),
            to: FileName::new("link.png").unwrap(),
        })
        .await
        .unwrap();
    assert_eq!(refused, Outcome::Taken);
    assert!(file.as_path().exists());

    desktop
        .carry_out(Task::Rename {
            file: file.clone(),
            to: FileName::new("b.png").unwrap(),
        })
        .await
        .unwrap();
    let moved = path(dir.path().join("b.png").to_str().unwrap());
    assert_eq!(
        desktop.kept_versions(&moved).len(),
        1,
        "Revert To still lists it"
    );
    assert_eq!(desktop.kept_versions(&file), vec![]);
}

#[tokio::test]
async fn duplicate_skips_a_dangling_symlink_and_leaves_no_hidden_file() {
    let dir = tempfile::tempdir().unwrap();
    let image = probed(dir.path(), "a.png", PNG);
    let elsewhere = dir.path().join("elsewhere");
    std::os::unix::fs::symlink(&elsewhere, dir.path().join("a copy.png")).unwrap();
    let (desktop, _) = desktop(dir.path(), vec![]);
    let outcome = desktop
        .carry_out(Task::Duplicate(image.source.path().clone()))
        .await
        .unwrap();
    assert_eq!(
        outcome,
        Outcome::Wrote(path(dir.path().join("a copy 2.png").to_str().unwrap()))
    );
    assert!(!elsewhere.exists(), "the link's target was not created");
    assert_eq!(std::fs::read(dir.path().join("a copy 2.png")).unwrap(), PNG);
    let hidden = std::fs::read_dir(dir.path())
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with('.')
        })
        .count();
    assert_eq!(hidden, 0);
}
