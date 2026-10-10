//! What a pane makes of each request of the viewer's window: every `HostRequest` is placed, and
//! the table below says where.

use anyview_core::{
    ByteLen, Edit, FileAction, FileHead, FileName, FilePath, FileStamp, Helper, MediaExport,
    ModTime, PageIndex, PixelLen, PixelSize, QuarterTurn, Resume, SniffStep, Source, TimeRange,
    sniff,
};
use anyview_pane::{
    EditRequest, ExportDraft, FileAccess, FileRequest, NaturalSize, Opened, PaneRequest, Rewind,
    TextSave, TypedText, VersionKey,
};
use anyview_ui::{HostRequest, Presentation, SizeBasis, family_of};
use std::collections::BTreeSet;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0";

fn opened() -> Opened {
    let name = FileName::new("a.png").unwrap();
    let SniffStep::Done(sniffed) = sniff(&FileHead::new(PNG), &name) else {
        panic!("a.png needs a look inside");
    };
    Opened {
        source: Source::new(
            FilePath::new("/p/a.png").unwrap(),
            FileStamp {
                len: ByteLen(PNG.len() as u64),
                modified: ModTime(1),
            },
        ),
        family: family_of(sniffed.kind()),
        sniffed,
        resume: Resume::Nothing,
        access: FileAccess::Writable,
    }
}

fn path(text: &str) -> FilePath {
    FilePath::new(text).unwrap()
}

/// A number for each request, from a match that names every one: a request added to the
/// vocabulary stops this compiling until the table has a row for it.
fn variant(request: &HostRequest) -> usize {
    match request {
        HostRequest::Opened(_) => 0,
        HostRequest::Run(_) => 1,
        HostRequest::PickFile => 2,
        HostRequest::CloseWindow => 3,
        HostRequest::Export(_) => 4,
        HostRequest::Trash => 5,
        HostRequest::Rename(_) => 6,
        HostRequest::Edit(_) => 7,
        HostRequest::SaveText(_) => 8,
        HostRequest::Rewind(_) => 9,
        HostRequest::RevertTo(_) => 10,
        HostRequest::SaveCopy(_) => 11,
        HostRequest::Remember(_) => 12,
        HostRequest::Watch(_) => 13,
        HostRequest::Unwatch => 14,
        HostRequest::Present(_) => 15,
        HostRequest::OpenFiles(_) => 16,
        HostRequest::Reveal(_) => 17,
        HostRequest::OpenUri(_) => 18,
        HostRequest::Provide(_) => 19,
        HostRequest::SizeWindow(_) => 20,
    }
}

const VARIANTS: usize = 21;

fn size() -> NaturalSize {
    NaturalSize::Points(PixelSize {
        width: PixelLen(600),
        height: PixelLen(400),
    })
}

fn file(request: FileRequest) -> Option<PaneRequest> {
    Some(PaneRequest::File(request))
}

#[test]
fn every_request_of_the_window_is_placed_as_the_table_says() {
    let shown = opened();
    let name = || TypedText::new("b.png");
    let draft = || ExportDraft::Media(MediaExport::Trim(TimeRange::WHOLE));
    let edit = || EditRequest::on_page(Edit::Rotate(QuarterTurn::Quarter), PageIndex(0));
    let version = || VersionKey::new("2026-10-10");
    let address = || "https://example.org".to_owned();
    let table: Vec<(HostRequest, Option<PaneRequest>)> = vec![
        (
            HostRequest::SizeWindow(SizeBasis::Natural(size())),
            Some(PaneRequest::SizeHint(size())),
        ),
        (HostRequest::SizeWindow(SizeBasis::Header), None),
        (HostRequest::SizeWindow(SizeBasis::Default), None),
        (HostRequest::CloseWindow, Some(PaneRequest::ClosePane)),
        (
            HostRequest::Present(Presentation::Mini),
            Some(PaneRequest::Detach(Presentation::Mini)),
        ),
        (HostRequest::Present(Presentation::Window), None),
        (HostRequest::Present(Presentation::Peek), None),
        (HostRequest::Present(Presentation::Background), None),
        (HostRequest::Present(Presentation::Pane), None),
        (
            HostRequest::PickFile,
            Some(PaneRequest::OpenElsewhere(Vec::new())),
        ),
        (
            HostRequest::OpenFiles(vec![path("/p/a.mkv")]),
            Some(PaneRequest::OpenElsewhere(vec![path("/p/a.mkv")])),
        ),
        (
            HostRequest::Opened(shown.clone()),
            Some(PaneRequest::Opened(shown)),
        ),
        (
            HostRequest::Remember(Resume::Nothing),
            Some(PaneRequest::Remember(Resume::Nothing)),
        ),
        (
            HostRequest::Watch(path("/p/a.png")),
            Some(PaneRequest::Watch(path("/p/a.png"))),
        ),
        (HostRequest::Unwatch, Some(PaneRequest::Unwatch)),
        (
            HostRequest::Run(FileAction::Share),
            file(FileRequest::Run(FileAction::Share)),
        ),
        (
            HostRequest::Export(draft()),
            file(FileRequest::Export(draft())),
        ),
        (HostRequest::Trash, file(FileRequest::Trash)),
        (
            HostRequest::Rename(name()),
            file(FileRequest::Rename(name())),
        ),
        (HostRequest::Edit(edit()), file(FileRequest::Edit(edit()))),
        (
            HostRequest::SaveText(TextSave::new(b"text".to_vec())),
            file(FileRequest::SaveText(TextSave::new(b"text".to_vec()))),
        ),
        (
            HostRequest::Rewind(Rewind::Undo),
            file(FileRequest::Rewind(Rewind::Undo)),
        ),
        (
            HostRequest::RevertTo(version()),
            file(FileRequest::RevertTo(version())),
        ),
        (
            HostRequest::SaveCopy(name()),
            file(FileRequest::SaveCopy(name())),
        ),
        (
            HostRequest::Reveal(path("/p/a.png")),
            file(FileRequest::Reveal(path("/p/a.png"))),
        ),
        (
            HostRequest::OpenUri(address()),
            file(FileRequest::OpenUri(address())),
        ),
        (
            HostRequest::Provide(Helper::VideoPlayback),
            file(FileRequest::Provide(Helper::VideoPlayback)),
        ),
    ];
    for (request, want) in &table {
        assert_eq!(
            PaneRequest::from_host(request.clone()),
            *want,
            "{request:?}"
        );
    }
    let placed: BTreeSet<usize> = table.iter().map(|(request, _)| variant(request)).collect();
    assert_eq!(
        placed,
        (0..VARIANTS).collect::<BTreeSet<usize>>(),
        "a request of the window has no row"
    );
}
