use super::support::{PDF, PNG, probed};
use crate::host::{Carry, Declined, Shown, Task, WindowTask, route};
use anyview_core::{
    ExportChoice, FileAction, FileName, MediaExport, PdfExport, RasterExport, Resume, TextExport,
};
use anyview_export::DocumentExport;
use anyview_ui::{ExportDraft, ExportFamily, HostRequest, Presentation, TypedText};

/// The export the sheet confirms for a trim of the whole recording.
fn media_export() -> ExportDraft {
    ExportDraft::Media(MediaExport::Trim(anyview_core::TimeRange::WHOLE))
}

/// The export a sheet opens on for a format: its first kind with that kind's default options.
fn first<E: ExportChoice>() -> E {
    E::default_for(E::kinds()[0])
}

fn requests_of_a_window(
    image: &anyview_ui::Probed,
    pdf: &anyview_ui::Probed,
    notes: &anyview_ui::Probed,
    recording: &anyview_ui::Probed,
) -> Vec<(&'static str, Shown, HostRequest, Carry)> {
    let open = |probed: &anyview_ui::Probed| {
        route(Shown::default(), HostRequest::Opened(probed.clone())).0
    };
    let path = |probed: &anyview_ui::Probed| probed.source.path().clone();
    vec![
        (
            "a newly shown file is recorded as viewed",
            Shown::default(),
            HostRequest::Opened(image.clone()),
            Carry::Desktop(Task::RecordView(image.clone())),
        ),
        (
            "close is the window's own",
            open(image),
            HostRequest::CloseWindow,
            Carry::Window(WindowTask::Close),
        ),
        (
            "reveal names the file shown",
            open(image),
            HostRequest::Run(FileAction::RevealInFolder),
            Carry::Desktop(Task::Reveal(path(image))),
        ),
        (
            "copy path is text for the window's clipboard",
            open(image),
            HostRequest::Run(FileAction::CopyPath),
            Carry::Window(WindowTask::CopyText(
                image.source.path().as_path().to_string_lossy().into_owned(),
            )),
        ),
        (
            "share names the file",
            open(image),
            HostRequest::Run(FileAction::Share),
            Carry::Desktop(Task::Share(path(image))),
        ),
        (
            "duplicate names the file",
            open(image),
            HostRequest::Run(FileAction::Duplicate),
            Carry::Desktop(Task::Duplicate(path(image))),
        ),
        (
            "print goes through for a pdf",
            open(pdf),
            HostRequest::Run(FileAction::Print),
            Carry::Desktop(Task::Print(pdf.clone())),
        ),
        (
            "print goes through for a picture, which is laid out as a pdf first",
            open(image),
            HostRequest::Run(FileAction::Print),
            Carry::Desktop(Task::Print(image.clone())),
        ),
        (
            "print goes through for a text document",
            open(notes),
            HostRequest::Run(FileAction::Print),
            Carry::Desktop(Task::Print(notes.clone())),
        ),
        (
            "a recording is not laid out on paper",
            open(recording),
            HostRequest::Run(FileAction::Print),
            Carry::Declined(Declined::NotPrintable),
        ),
        (
            "the confirmed trash names the file",
            open(image),
            HostRequest::Trash,
            Carry::Desktop(Task::Trash(path(image))),
        ),
        (
            "the trash action, were it handed over, is the same task",
            open(image),
            HostRequest::Run(FileAction::MoveToTrash),
            Carry::Desktop(Task::Trash(path(image))),
        ),
        (
            "a typed name renames the file shown",
            open(image),
            HostRequest::Rename(TypedText::new("b.png")),
            Carry::Desktop(Task::Rename {
                file: path(image),
                to: FileName::new("b.png").unwrap(),
            }),
        ),
        (
            "a typed name with a slash is no name",
            open(image),
            HostRequest::Rename(TypedText::new("a/b.png")),
            Carry::Declined(Declined::NotAFileName),
        ),
        (
            "the place is kept against the file's source",
            open(image),
            HostRequest::Remember(Resume::Nothing),
            Carry::Desktop(Task::Remember {
                source: image.source.clone(),
                resume: Resume::Nothing,
            }),
        ),
        (
            "the window is told when the file it shows changes",
            open(image),
            HostRequest::Watch(path(image)),
            Carry::Window(WindowTask::Watch(path(image))),
        ),
        (
            "and is told nothing once it shows none",
            open(image),
            HostRequest::Unwatch,
            Carry::Window(WindowTask::Unwatch),
        ),
        (
            "a link's web address is opened by the desktop",
            open(pdf),
            HostRequest::OpenUri("https://example.com/".to_owned()),
            Carry::Desktop(Task::OpenLink("https://example.com/".to_owned())),
        ),
        (
            "a request about the file before there is one",
            Shown::default(),
            HostRequest::Run(FileAction::RevealInFolder),
            Carry::Declined(Declined::NoFileShown),
        ),
        (
            "open is already done",
            open(image),
            HostRequest::Run(FileAction::Open),
            Carry::Declined(Declined::AlreadyOpen),
        ),
        (
            "choosing a file asks the desktop's dialog, with or without a file shown",
            open(image),
            HostRequest::PickFile,
            Carry::Desktop(Task::PickFile),
        ),
        (
            "so does the welcome window, which shows none",
            Shown::default(),
            HostRequest::PickFile,
            Carry::Desktop(Task::PickFile),
        ),
        (
            "a notice's Show in Folder reveals the file it names, not the one shown",
            open(image),
            HostRequest::Reveal(path(pdf)),
            Carry::Desktop(Task::Reveal(path(pdf))),
        ),
        (
            "what the welcome window is given opens in windows of their own",
            Shown::default(),
            HostRequest::OpenFiles(vec![path(image), path(pdf)]),
            Carry::Window(WindowTask::OpenFiles(vec![path(image), path(pdf)])),
        ),
        (
            "copying the file itself needs more than text",
            open(image),
            HostRequest::Run(FileAction::CopyFile),
            Carry::Declined(Declined::CopyFile),
        ),
        (
            "a flip asked of the host, which the viewer does only for a file with no flip, is declined",
            open(image),
            HostRequest::Run(FileAction::FlipHorizontal),
            Carry::Declined(Declined::Edit),
        ),
        (
            "background playback hands the probed file, with the place it was left, to the desktop",
            open(image),
            HostRequest::Run(FileAction::PlayInBackground),
            Carry::Desktop(Task::PlayInBackground(image.clone())),
        ),
        (
            "the mini window is a window made again for the file",
            open(image),
            HostRequest::Run(FileAction::PlayInMiniWindow),
            Carry::Window(WindowTask::Reopen(Presentation::Mini)),
        ),
        (
            "a rename without its sheet",
            open(image),
            HostRequest::Run(FileAction::Rename),
            Carry::Declined(Declined::NeedsSheet),
        ),
        (
            "becoming the mini window is making one",
            open(image),
            HostRequest::Present(Presentation::Mini),
            Carry::Window(WindowTask::Reopen(Presentation::Mini)),
        ),
        (
            "becoming a window again from the mini window is making one",
            open(image),
            HostRequest::Present(Presentation::Window),
            Carry::Window(WindowTask::Reopen(Presentation::Window)),
        ),
        (
            "a window does not become a quick look",
            open(image),
            HostRequest::Present(Presentation::Peek),
            Carry::Declined(Declined::Present),
        ),
        (
            "nor a background session: that is a file action",
            open(image),
            HostRequest::Present(Presentation::Background),
            Carry::Declined(Declined::Present),
        ),
        (
            "a recording's export is written beside it",
            open(image),
            HostRequest::Export(media_export()),
            Carry::Desktop(Task::ExportMedia {
                file: image.clone(),
                choice: MediaExport::Trim(anyview_core::TimeRange::WHOLE),
            }),
        ),
        (
            "an image's export is written by the export crate",
            open(image),
            HostRequest::Export(ExportDraft::first_of(ExportFamily::Raster).unwrap()),
            Carry::Desktop(Task::ExportDocument {
                file: image.clone(),
                choice: DocumentExport::Raster(first::<RasterExport>()),
            }),
        ),
        (
            "so is a pdf's",
            open(pdf),
            HostRequest::Export(ExportDraft::first_of(ExportFamily::Pdf).unwrap()),
            Carry::Desktop(Task::ExportDocument {
                file: pdf.clone(),
                choice: DocumentExport::Pdf(first::<PdfExport>()),
            }),
        ),
        (
            "and a text document's",
            open(notes),
            HostRequest::Export(ExportDraft::first_of(ExportFamily::Text).unwrap()),
            Carry::Desktop(Task::ExportDocument {
                file: notes.clone(),
                choice: DocumentExport::Text(first::<TextExport>()),
            }),
        ),
        (
            "an export with no file shown has no file to write",
            Shown::default(),
            HostRequest::Export(ExportDraft::first_of(ExportFamily::Raster).unwrap()),
            Carry::Declined(Declined::NoFileShown),
        ),
        (
            "the export action is the sheet's to answer",
            open(image),
            HostRequest::Run(FileAction::Export),
            Carry::Declined(Declined::NeedsSheet),
        ),
    ]
}

#[test]
fn every_request_becomes_the_task_that_carries_it_out() {
    let dir = tempfile::tempdir().unwrap();
    let image = probed(dir.path(), "a.png", PNG);
    let pdf = probed(dir.path(), "a.pdf", PDF);
    let notes = probed(dir.path(), "a.md", b"# Notes\n");
    let recording = probed(dir.path(), "a.mp3", b"ID3\x04\0\0\0\0\0\0");
    for (name, shown, request, want) in requests_of_a_window(&image, &pdf, &notes, &recording) {
        let (_, carry) = route(shown, request);
        assert_eq!(carry, want, "{name}");
    }
}

#[test]
fn the_file_shown_is_the_last_one_opened_and_follows_a_move() {
    let dir = tempfile::tempdir().unwrap();
    let first = probed(dir.path(), "a.png", PNG);
    let second = probed(dir.path(), "b.png", PNG);
    let (shown, _) = route(Shown::default(), HostRequest::Opened(first));
    let (shown, _) = route(shown, HostRequest::Opened(second.clone()));
    assert_eq!(shown.file(), Some(&second), "the second replaced the first");

    let renamed = second
        .source
        .path()
        .parent()
        .unwrap()
        .as_path()
        .join("c.png");
    let moved = shown.moved_to(super::support::path(renamed.to_str().unwrap()));
    assert_eq!(
        moved
            .file()
            .map(|probed| probed.source.path().as_path().to_path_buf()),
        Some(renamed),
        "the name changed"
    );
    assert_eq!(
        moved.file().map(|probed| probed.source.stamp()),
        Some(second.source.stamp()),
        "and nothing else did"
    );
}

#[test]
fn the_window_says_where_it_is_and_the_host_keeps_it_for_the_files_next_task() {
    let dir = tempfile::tempdir().unwrap();
    let image = probed(dir.path(), "a.png", PNG);
    let (shown, _) = route(Shown::default(), HostRequest::Opened(image.clone()));
    let place = Resume::Media {
        at: anyview_core::MediaTime::from_secs(90),
        volume: anyview_core::Volume::FULL,
        audio: anyview_core::TrackChoice::Auto,
        subtitles: anyview_core::TrackChoice::Off,
    };
    let (shown, carry) = route(shown, HostRequest::Remember(place.clone()));
    assert_eq!(
        carry,
        Carry::Desktop(Task::Remember {
            source: image.source.clone(),
            resume: place.clone(),
        })
    );
    assert_eq!(
        shown.file().map(|probed| probed.resume.clone()),
        Some(place),
        "a background session or a new window starts from here"
    );
}
