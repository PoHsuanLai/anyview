//! What the window carries out of the root machine's outputs, and what it needs to do it.

use super::super::arrive::shown_path;
use super::super::editing;
use super::super::pane::PaneSeat;
use super::super::shelf::{Dispatch, Shelf};
use super::file::{opened, preload, reload, remember_on_leaving};
use super::stage::staged;
use crate::io::{Edge, HostRequest, Job};
use crate::{
    ChromeOut, EditRequest, LoadOut, PaletteOut, PanelOut, PresentationOut, SheetIn, SheetOut,
    Stage, TypedText, ViewerIn, ViewerOut,
};
use anyview_core::{Edit, FilePath};
use dioxus::prelude::*;
use ds::stack::toast_hub::ToastHub;
use ds_blitz::Gpu;

/// What carrying out an output needs: the shelf it writes to, the edge, the GPU a picture is
/// uploaded to, and the machine itself (set once it exists, since the machine is built with this).
#[derive(Clone)]
pub(in crate::views) struct Carry {
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
    pub(in crate::views) fn dispatch(&self) -> Option<Dispatch> {
        self.machine.cloned()
    }
}
/// Do what `out` asks.
pub(in crate::views) fn carry_out(out: ViewerOut, c: &Carry) {
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
        ViewerOut::Sheet(SheetOut::Edit(request)) => edit(c, request),
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
        ViewerOut::Edit(request) => edit(c, request),
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
/// The host is asked to save `request`. What is written on a PDF is held by the markup, not by the
/// request: the markup says what to save when the save is asked.
fn edit(c: &Carry, request: EditRequest) {
    if request.edit != Edit::Markup {
        c.edge.request(HostRequest::Edit(request));
        return;
    }
    if let Some(dispatch) = c.dispatch() {
        let save = dispatch.machine.state().peek().markup.save();
        c.edge.request(HostRequest::SaveMarkup(save));
    }
}
/// The name proposed for a copy of `file`: `name copy.ext`, beside it.
pub(super) fn copy_name(file: &FilePath) -> TypedText {
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
