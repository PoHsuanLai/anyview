use super::support::{PDF, PNG, probed};
use crate::host::{Carry, Declined, Shown, Task, WindowTask, route};
use anyview_core::{FileAction, FileName, Resume};
use anyview_ui::{ExportDraft, HostRequest, Presentation, TypedText};

fn requests_of_a_window(
    image: &anyview_ui::Probed,
    pdf: &anyview_ui::Probed,
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
            "open with hands over the probed file, whose type picks the program",
            open(image),
            HostRequest::Run(FileAction::OpenWith),
            Carry::Desktop(Task::OpenWith(image.clone())),
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
            "print of a picture waits for an export to pdf",
            open(image),
            HostRequest::Run(FileAction::Print),
            Carry::Declined(Declined::PrintNeedsPdf),
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
            "a link's web address waits for the edge to have a way to open it",
            open(pdf),
            HostRequest::OpenUri("https://example.com/".to_owned()),
            Carry::Declined(Declined::OpenUri),
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
            "choosing a file needs a chooser",
            open(image),
            HostRequest::PickFile,
            Carry::Declined(Declined::PickFile),
        ),
        (
            "copying the file itself needs more than text",
            open(image),
            HostRequest::Run(FileAction::CopyFile),
            Carry::Declined(Declined::CopyFile),
        ),
        (
            "an edit in place is not wired",
            open(image),
            HostRequest::Run(FileAction::FlipHorizontal),
            Carry::Declined(Declined::Edit),
        ),
        (
            "background playback needs the player",
            open(image),
            HostRequest::Run(FileAction::PlayInBackground),
            Carry::Declined(Declined::Playback),
        ),
        (
            "a rename without its sheet",
            open(image),
            HostRequest::Run(FileAction::Rename),
            Carry::Declined(Declined::NeedsSheet),
        ),
        (
            "the mini window is the window layer's",
            open(image),
            HostRequest::Present(Presentation::Mini),
            Carry::Declined(Declined::Present),
        ),
        (
            "an export is not wired",
            open(image),
            HostRequest::Export(ExportDraft::first_of(anyview_ui::ExportFamily::Raster).unwrap()),
            Carry::Declined(Declined::Export),
        ),
    ]
}

#[test]
fn every_request_becomes_the_task_that_carries_it_out() {
    let dir = tempfile::tempdir().unwrap();
    let image = probed(dir.path(), "a.png", PNG);
    let pdf = probed(dir.path(), "a.pdf", PDF);
    for (name, shown, request, want) in requests_of_a_window(&image, &pdf) {
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
