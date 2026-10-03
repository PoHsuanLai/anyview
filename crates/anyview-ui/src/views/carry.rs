//! What the window does with each output of the root machine. Nothing here decides: an output says
//! what is wanted, and this hands it to a worker, a signal the views read, or the host.

use super::shelf::{Dispatch, Shelf};
use crate::families::top_for;
use crate::io::{Edge, HostRequest, Job, Preloaded};
use crate::{
    ChromeOut, FindOut, HitIndex, LoadOut, PaletteOut, PanelOut, PresentationOut, SheetOut, Stage,
    StageIn, StageOut, TextIn, TextStage, Ticket, TypedText, ViewerIn, ViewerOut,
};
use anyview_core::{FilePath, Neighbours, Resume};
use dioxus::prelude::*;
use ds::motion::detail::operation::{Operation, PendingToken};
use ds_blitz::Gpu;

/// What carrying out an output needs: the shelf it writes to, the edge, the GPU a picture is
/// uploaded to, and the machine itself (set once it exists, since the machine is built with this).
#[derive(Clone)]
pub(super) struct Carry {
    pub shelf: Shelf,
    pub edge: Edge,
    pub gpu: Gpu,
    pub machine: CopyValue<Option<Dispatch>>,
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
        ViewerOut::Palette(PaletteOut::Opened) => shelf.query.set(TypedText::EMPTY),
        ViewerOut::Palette(PaletteOut::Closed | PaletteOut::Run(_))
        | ViewerOut::Panel(PanelOut::Show(_) | PanelOut::Hide)
        | ViewerOut::Sheet(SheetOut::Opened | SheetOut::Closed) => {}
        ViewerOut::Preload(neighbours) => preload(c, &neighbours),
        ViewerOut::Stage(out) => staged(c, &out),
        ViewerOut::Sheet(SheetOut::Export(draft)) => c.edge.request(HostRequest::Export(draft)),
        ViewerOut::Sheet(SheetOut::Trash) => c.edge.request(HostRequest::Trash),
        ViewerOut::Sheet(SheetOut::Rename(name)) => c.edge.request(HostRequest::Rename(name)),
        ViewerOut::Presentation(PresentationOut::Become(presentation)) => {
            c.edge.request(HostRequest::Present(presentation));
        }
        ViewerOut::Run(action) => c.edge.request(HostRequest::Run(action)),
        ViewerOut::PickFile => c.edge.request(HostRequest::PickFile),
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
    shelf.loaded.set(None);
    shelf.peeked.set(None);
    shelf.lines.set(None);
    shelf.hits.set(None);
    shelf.pdf.reset();
    shelf.left_at.set(Resume::Nothing);
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
        | Resume::Text { .. }) => remember(c, &placed),
    }
}

/// The open file, held as it was left: its document and where the person was in it, so coming
/// back to it needs no decode and no search for the place.
fn keep_the_one_left(c: &Carry) {
    let mut shelf = c.shelf;
    let (Some((_, doc)), Some(probed)) = (
        shelf.loaded.peek().clone(),
        shelf.probe.peek().found().cloned(),
    ) else {
        return;
    };
    let left = shelf.left_at.peek().clone();
    let resume = match left {
        Resume::Nothing => probed.resume.clone(),
        placed @ (Resume::Raster { .. }
        | Resume::Pdf { .. }
        | Resume::Media { .. }
        | Resume::Text { .. }) => placed,
    };
    shelf.preloads.write().stash(Preloaded {
        probed: crate::io::Probed { resume, ..probed },
        doc,
    });
}

/// The same file is probed again: what is on screen stays until the new copy lands.
fn reload(c: &Carry, ticket: Ticket, path: FilePath) {
    let mut shelf = c.shelf;
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
        let link = c.edge.link(c.gpu.handle());
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

/// Keep `resume` as where the person is in the open file: the host knows which file that is
/// from the `Opened` request it was sent.
fn remember(c: &Carry, resume: &Resume) {
    c.edge.request(HostRequest::Remember(resume.clone()));
}

/// Search the open file for `query`. A first frame is only the start of the file, so only the
/// full open is searched; the stage asks again when it lands.
fn search(c: &Carry, query: &TypedText) {
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
    let Stage::Text(TextStage::Reading { place } | TextStage::Finding { place, .. }) =
        dispatch.machine.state().peek().stage.clone()
    else {
        return;
    };
    let page = dispatch.params().stage.text.extent.page.0;
    let top = top_for(hit.line, place.line, page);
    if top != place.line {
        dispatch.send(ViewerIn::Stage(StageIn::Text(TextIn::Scroll(top))));
    }
}
