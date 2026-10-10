//! What the window does with each output of the root machine. Nothing here decides: an output says
//! what is wanted, and this hands it to a worker, a signal the views read, or the host.

use super::arrive::shown_path;
use super::editing;
use super::pane::PaneSeat;
use super::shelf::{Dispatch, Doc, Shelf};
use crate::families::{Leaving, top_for};
use crate::io::{Edge, HostRequest, Job, Preloaded};
use crate::{
    ChromeOut, FindOut, HitIndex, LoadOut, MediaOut, PaletteOut, PanelOut, PdfOut, PresentationOut,
    SheetIn, SheetOut, Stage, StageIn, StageOut, TextIn, TextOut, Ticket, TypedText, ViewerIn,
    ViewerOut,
};
use anyview_core::{FilePath, Neighbours, Resume};
use dioxus::prelude::*;
use ds::motion::detail::operation::{Operation, PendingToken};
use ds::stack::toast_hub::ToastHub;
use ds_blitz::Gpu;

/// What carrying out an output needs: the shelf it writes to, the edge, the GPU a picture is
/// uploaded to, and the machine itself (set once it exists, since the machine is built with this).
#[derive(Clone)]
pub(super) struct Carry {
    pub shelf: Shelf,
    pub edge: Edge,
    pub gpu: Gpu,
    pub toasts: ToastHub,
    pub machine: CopyValue<Option<Dispatch>>,
    /// Where a pane sits in its host: the one place Esc gives the keyboard back to.
    pub seat: Option<PaneSeat>,
}

impl Carry {
    /// The machine, once the window has built it.
    pub(super) fn dispatch(&self) -> Option<Dispatch> {
        self.machine.cloned()
    }
}

/// Do what `out` asks.
pub(super) fn carry_out(out: ViewerOut, c: &Carry) {
    let mut shelf = c.shelf;
    match out {
        ViewerOut::Probe { ticket, path } => opened(c, ticket, path),
        ViewerOut::Reload { ticket, path } => reload(c, ticket, path),
        ViewerOut::ListFolder(path) => {
            shelf.folder.set(Some(path.clone()));
            c.edge.submit(Job::Folder { path });
        }
        ViewerOut::Load(LoadOut::Open(ticket)) => shelf.opening.set(Some(ticket)),
        ViewerOut::Load(LoadOut::Peek(ticket)) => shelf.peeking.set(Some(ticket)),
        ViewerOut::Load(
            LoadOut::Probe(_)
            | LoadOut::Cancel(_)
            | LoadOut::UseStage(_)
            | LoadOut::ShowFirstFrame(_)
            | LoadOut::ShowFull(_),
        ) => {}
        ViewerOut::Chrome(ChromeOut::Fade { to, .. }) => shelf.chrome.set(to),
        ViewerOut::Palette(PaletteOut::Opened | PaletteOut::Closed | PaletteOut::Run(_))
        | ViewerOut::Panel(PanelOut::Show(_) | PanelOut::Hide)
        | ViewerOut::Sheet(
            SheetOut::Opened
            | SheetOut::Closed
            | SheetOut::Picture(_)
            | SheetOut::Save(_)
            | SheetOut::Discard(_),
        ) => {}
        ViewerOut::Sheet(SheetOut::Replace) => editing::save(c),
        ViewerOut::Preload(neighbours) => preload(c, &neighbours),
        ViewerOut::Stage(out) => staged(c, &out),
        ViewerOut::Sheet(SheetOut::Export(draft)) => {
            c.edge.request(HostRequest::Export(draft));
        }
        ViewerOut::Sheet(SheetOut::Edit(request)) => c.edge.request(HostRequest::Edit(request)),
        ViewerOut::Sheet(SheetOut::Trash) => c.edge.request(HostRequest::Trash),
        ViewerOut::Sheet(SheetOut::Rename(name)) => c.edge.request(HostRequest::Rename(name)),
        ViewerOut::Sheet(SheetOut::SaveCopy(name)) => c.edge.request(HostRequest::SaveCopy(name)),
        ViewerOut::Sheet(SheetOut::Revert(version)) => {
            c.edge.request(HostRequest::RevertTo(version));
        }
        ViewerOut::Sheet(SheetOut::Provide(helper)) => {
            c.edge.request(HostRequest::Provide(helper));
        }
        ViewerOut::Sheet(SheetOut::Reopen) => {
            if let (Some(path), Some(dispatch)) = (shown_path(c), c.dispatch()) {
                dispatch.send(ViewerIn::Reload(path));
            }
        }
        ViewerOut::Edit(request) => c.edge.request(HostRequest::Edit(request)),
        ViewerOut::Rewind(rewind) => c.edge.request(HostRequest::Rewind(rewind)),
        ViewerOut::ListVersions => {
            if let Some(path) = shown_path(c) {
                c.edge.submit(Job::Versions { path });
            }
        }
        ViewerOut::NameRename => {
            if let (Some(path), Some(dispatch)) = (shown_path(c), c.dispatch()) {
                let current = path
                    .file_name()
                    .map_or_else(String::new, |name| name.as_str().to_owned());
                dispatch.send(ViewerIn::Sheet(SheetIn::AskRename(TypedText::new(current))));
            }
        }
        ViewerOut::NameCopy => {
            if let (Some(path), Some(dispatch)) = (shown_path(c), c.dispatch()) {
                dispatch.send(ViewerIn::Sheet(SheetIn::AskSaveCopy(copy_name(&path))));
            }
        }
        ViewerOut::Presentation(PresentationOut::Become(presentation)) => {
            c.edge.request(HostRequest::Present(presentation));
        }
        ViewerOut::Run(action) => c.edge.request(HostRequest::Run(action)),
        ViewerOut::PickFile => c.edge.request(HostRequest::PickFile),
        ViewerOut::Unfocus => {
            if let Some(seat) = c.seat {
                seat.unfocus.call(());
            }
        }
        ViewerOut::CloseWindow => {
            remember_on_leaving(c);
            c.edge.request(HostRequest::Unwatch);
            c.edge.request(HostRequest::CloseWindow);
        }
    }
}

/// A new file is wanted: the one on screen is kept as it was left, and the new one comes from the
/// files opened ahead when it is among them (and is checked against the disk, since it may have
/// changed since), otherwise from a probe.
fn opened(c: &Carry, ticket: Ticket, path: FilePath) {
    let mut shelf = c.shelf;
    remember_on_leaving(c);
    keep_the_one_left(c);
    shelf.wanted.set(Some(path.clone()));
    shelf.sizing.set(Some(ticket));
    shelf.loaded.set(None);
    shelf.peeked.set(None);
    shelf.lines.set(None);
    shelf.hits.set(None);
    shelf.pdf.reset();
    shelf.media.reset();
    shelf.left_at.set(Resume::Nothing);
    editing::forget(c);
    shelf
        .operation
        .set(Operation::Running(PendingToken::start()));
    let ahead = shelf.preloads.write().take(&path);
    match ahead {
        Some(Preloaded { probed, doc }) => {
            shelf.loaded.set(Some((ticket, doc)));
            shelf
                .probe
                .set(super::session::Probe::Arrived(ticket, probed));
            c.edge.submit(Job::Stat { path });
        }
        None => {
            shelf.probe.set(super::session::Probe::Pending(ticket));
            c.edge.submit(Job::Probe { ticket, path });
        }
    }
}

/// The place the person last settled at is said once more as they leave the file or the window, so
/// that the host has the final one whatever it did with the others (the host knows which file from
/// the `Opened` request it was sent, and the next file's has not come yet).
fn remember_on_leaving(c: &Carry) {
    let left = c.shelf.left_at.peek().clone();
    match left {
        Resume::Nothing => {}
        placed @ (Resume::Raster { .. }
        | Resume::Pdf { .. }
        | Resume::Media { .. }
        | Resume::Text { .. }
        | Resume::Book { .. }) => remember(c, &placed),
    }
}

/// The open file, held as it was left: its document and where the person was in it, so coming
/// back to it needs no decode and no search for the place.
fn keep_the_one_left(c: &Carry) {
    let mut shelf = c.shelf;
    // A file written while it was edited is not what its document holds: it is opened afresh.
    if *shelf.edit.doc.peek() == Doc::Stale {
        return;
    }
    let (Some((_, doc)), Some(probed)) = (
        shelf.loaded.peek().clone(),
        shelf.probe.peek().found().cloned(),
    ) else {
        return;
    };
    match doc.view().leaving() {
        Leaving::Keep => {}
        // Held, a recording would play on unseen.
        Leaving::Release => return,
    }
    let left = shelf.left_at.peek().clone();
    let resume = match left {
        Resume::Nothing => probed.resume.clone(),
        placed @ (Resume::Raster { .. }
        | Resume::Pdf { .. }
        | Resume::Media { .. }
        | Resume::Text { .. }
        | Resume::Book { .. }) => placed,
    };
    shelf.preloads.write().stash(Preloaded {
        probed: crate::io::Opened { resume, ..probed },
        doc,
    });
}

/// The same file is probed again: what is on screen stays until the new copy lands.
fn reload(c: &Carry, ticket: Ticket, path: FilePath) {
    let mut shelf = c.shelf;
    shelf.wanted.set(Some(path.clone()));
    let was = shelf.probe.peek().found().cloned();
    shelf.probe.set(match was {
        Some(was) => super::session::Probe::Reprobing(ticket, was),
        None => super::session::Probe::Pending(ticket),
    });
    c.edge.submit(Job::Probe { ticket, path });
}

/// Open the files next to the open one ahead of time, on the pool's quiet lane. Nothing is opened
/// while the window has no device to upload a picture to.
fn preload(c: &Carry, neighbours: &Neighbours) {
    let mut shelf = c.shelf;
    let around: Vec<FilePath> = [neighbours.previous.clone(), neighbours.next.clone()]
        .into_iter()
        .flatten()
        .collect();
    let wanted = if c.gpu.device().is_some() {
        around
    } else {
        Vec::new()
    };
    let fetch = shelf.preloads.write().want(&wanted);
    for path in fetch {
        let link = c.edge.link_for_preload(c.gpu.handle());
        c.edge.submit(Job::Preload { path, link });
    }
}

/// What a stage asks of the window: where the person is, and its search.
fn staged(c: &Carry, out: &StageOut) {
    let mut shelf = c.shelf;
    if let Some(resume) = out.remembered() {
        shelf.left_at.set(resume.clone());
        remember(c, resume);
    }
    if let StageOut::Media(media) = out {
        media_out(c, media);
        return;
    }
    match out {
        StageOut::Text(TextOut::BeginEdit) => return editing::begin(c),
        StageOut::Text(TextOut::EndEdit) => return editing::end(c),
        StageOut::Text(TextOut::Save) => return editing::save(c),
        StageOut::Text(TextOut::Wrapped(wrap)) => return editing::wrapped(c, *wrap),
        StageOut::Text(
            TextOut::Remember(_) | TextOut::ScrollTo(_) | TextOut::Show(_) | TextOut::Find(_),
        )
        | StageOut::Raster(_)
        | StageOut::Pdf(_)
        | StageOut::Media(_)
        | StageOut::Table(_)
        | StageOut::Tree(_) => {}
    }
    if let StageOut::Pdf(PdfOut::Edit(request)) = out {
        c.edge.request(HostRequest::Edit(*request));
        return;
    }
    if let StageOut::Pdf(pdf) = out {
        // The pages and their search are drawn from the PDF shelf, not the text's.
        shelf.pdf.carry(pdf.clone());
        return;
    }
    match out.find() {
        Some(FindOut::Search(query)) => search(c, query),
        Some(FindOut::Clear) => shelf.hits.set(None),
        Some(FindOut::ShowHit(index)) => reveal(c, *index),
        None => {}
    }
}

/// What the media stage asks: a command for the player, or a trim mark to keep for the export.
fn media_out(c: &Carry, out: &MediaOut) {
    match out {
        MediaOut::Command(command) => {
            if let Some((_, doc)) = c.shelf.shown_now()
                && let Some(line) = doc.view().line()
            {
                line.send(*command);
            }
        }
        MediaOut::Marked { edge, at } => c.shelf.media.mark(*edge, *at),
        MediaOut::Buffering(_) | MediaOut::VolumeChanged(_) | MediaOut::TracksChanged => {}
    }
}

/// Keep `resume` as where the person is in the open file: the host knows which file that is
/// from the `Opened` request it was sent.
fn remember(c: &Carry, resume: &Resume) {
    c.edge.request(HostRequest::Remember(resume.clone()));
}

/// Search the open file for `query`. A first frame is only the start of the file, so only the
/// full open is searched; the stage asks again when it lands.
fn search(c: &Carry, query: &TypedText) {
    if editing::searches_text(c) {
        return editing::search(c, query);
    }
    let Some((ticket, doc)) = c.shelf.loaded.peek().clone() else {
        return;
    };
    if let Some(job) = doc.view().search(ticket, query) {
        c.edge.submit(job);
    }
}

/// Scroll to the hit numbered `index` when it is not already in the page.
fn reveal(c: &Carry, index: HitIndex) {
    let (Some(dispatch), Some(found)) = (c.dispatch(), c.shelf.hits.peek().clone()) else {
        return;
    };
    let Some(hit) = found.0.get(index) else {
        return;
    };
    let Stage::Text(text) = dispatch.machine.state().peek().stage.clone() else {
        return;
    };
    let place = text.place();
    let page = dispatch.params().stage.text.extent.page.0;
    let top = top_for(hit.line, place.line, page);
    if top != place.line {
        dispatch.send(ViewerIn::Stage(StageIn::Text(TextIn::Scroll(top))));
    }
}

/// The name proposed for a copy of `file`: `name copy.ext`, beside it.
fn copy_name(file: &FilePath) -> TypedText {
    let path = file.as_path();
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let extension = path
        .extension()
        .map(|extension| format!(".{}", extension.to_string_lossy()))
        .unwrap_or_default();
    TypedText::new(format!("{stem} copy{extension}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_copy_is_named_after_its_file() {
        // name, file, proposed name
        const CASES: &[(&str, &str, &str)] = &[
            ("with an extension", "/p/photo.png", "photo copy.png"),
            ("with none", "/p/notes", "notes copy"),
            ("with two dots", "/p/a.tar.gz", "a.tar copy.gz"),
        ];
        for (name, file, want) in CASES {
            let proposed = copy_name(&FilePath::new(file).unwrap());
            assert_eq!(proposed.as_str(), *want, "{name}");
        }
    }
}
