//! What the window holds besides the machines' states: the results of effects, kept so the views
//! can draw them and so a late result for a file the person left is dropped by its ticket. Every
//! field is a signal, so the shelf is `Copy` and any handler or effect can take it.

use super::preloads::Preloads;
use super::session::{Live, PictureOffer, Probe, params};
use crate::families::{
    Area, FoundHits, Held, LineWindow, LoadedDoc, MediaShelf, PdfShelf, use_media_shelf,
    use_pdf_shelf,
};
use crate::{PaneChrome, PlatformAbilities, Presentation, Ticket, Viewer, ViewerIn, ViewerParams};
use anyview_core::{FilePath, Resume};
use anyview_machines::seam::WrapChoices;
use dioxus::prelude::*;
use ds::host::measure::use_rect;
use ds::machine::MachineRef;
use ds::motion::detail::level::{Level, use_level};
use ds::motion::detail::operation::Operation;
use ds::prelude::{Scale, Shown};

/// The window's held results.
#[derive(Clone, Copy)]
pub(super) struct Shelf {
    /// Where the probe of the load in flight stands.
    pub probe: Signal<Probe>,
    /// The load whose full open is wanted: the window submits it once it can.
    pub opening: Signal<Option<Ticket>>,
    /// The load whose first frame is wanted.
    pub peeking: Signal<Option<Ticket>>,
    /// The document the full open made.
    pub loaded: Signal<Option<(Ticket, LoadedDoc)>>,
    /// The document of the first frame, shown until `loaded` is.
    pub peeked: Signal<Option<(Ticket, LoadedDoc)>>,
    /// The lines of a text last read.
    pub lines: Signal<Option<Held<LineWindow>>>,
    /// The places the current find found.
    pub hits: Signal<Option<Held<FoundHits>>>,
    /// What is held of an open PDF: the tiles, the hits, the thumbnails.
    pub pdf: PdfShelf,
    /// What is held of the recording that plays: the position, the lists, the trim marks.
    pub media: MediaShelf,
    /// The files opened ahead.
    pub preloads: Signal<Preloads>,
    /// The file the window last asked to open: what a failure screen is about when the probe did
    /// not find it.
    pub wanted: Signal<Option<FilePath>>,
    /// The file whose folder was asked for after a drop.
    pub folder: Signal<Option<FilePath>>,
    /// What the spinner shows while loading.
    pub operation: Signal<Operation>,
    /// Whether the chrome is faded in.
    pub chrome: Signal<Shown>,
    /// The window's motion level: what the desktop asks for, read when a step needs it.
    pub level: Level,
    /// The load of a different file whose landing sizes the window; a reload is not one.
    pub sizing: Signal<Option<Ticket>>,
    /// Where the person last said they were in the open file, kept for the file when it is left.
    pub left_at: Signal<Resume>,
    /// How the person last chose to wrap each kind of text, for the next file of the kind.
    pub wraps: Signal<WrapChoices>,
    /// The text being edited and what the window knows about its file.
    pub edit: EditShelf,
    /// What the platform can do: fixed for the window's life, so not a signal.
    pub platform: PlatformAbilities,
    /// How much chrome a pane draws: fixed for its life, and read by a pane alone.
    pub pane_chrome: PaneChrome,
}

/// Where a save of the edited text stands, as far as the file's stamp is concerned: while the
/// host writes, and until the stamp it left is read, a change of the file is the window's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Settle {
    /// No save of ours is under way: a change of the file is another program's.
    Idle,
    /// The host is writing.
    Saving,
    /// The host wrote; the stamp it left is being read.
    Reading,
}

/// Whether the document the window holds still reads the file as it was when it opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Doc {
    /// It does.
    Current,
    /// The file was written since: the lines it holds are the old ones, and the file is opened
    /// again when the editing ends.
    Stale,
}

/// What the window holds of the text being edited besides the text (which the stage's context
/// carries, as it is read by the views).
#[derive(Clone, Copy)]
pub(super) struct EditShelf {
    /// The text, once read.
    pub session: Signal<Option<anyview_text::Session>>,
    /// The handle on the edit surface.
    pub handle: ds::edit::handle::EditHandle,
    /// Where a save stands.
    pub settle: Signal<Settle>,
    /// The text the host is writing: its stamp, so the save marks that text and no later one.
    pub writing: Signal<Option<anyview_text::Revision>>,
    /// Whether the document is behind the file.
    pub doc: Signal<Doc>,
}

impl Shelf {
    /// A shelf with nothing on it.
    pub(super) fn empty(platform: PlatformAbilities) -> Shelf {
        Shelf {
            probe: use_signal(|| Probe::Idle),
            opening: use_signal(|| None),
            peeking: use_signal(|| None),
            loaded: use_signal(|| None),
            peeked: use_signal(|| None),
            lines: use_signal(|| None),
            hits: use_signal(|| None),
            pdf: use_pdf_shelf(),
            media: use_media_shelf(),
            preloads: use_signal(Preloads::default),
            wanted: use_signal(|| None),
            folder: use_signal(|| None),
            operation: use_signal(|| Operation::Idle),
            chrome: use_signal(|| Shown::Hidden),
            level: use_level(),
            sizing: use_signal(|| None),
            left_at: use_signal(|| Resume::Nothing),
            wraps: use_signal(WrapChoices::default),
            edit: EditShelf {
                session: use_signal(|| None),
                handle: ds::edit::handle::use_edit_handle(),
                settle: use_signal(|| Settle::Idle),
                writing: use_signal(|| None),
                doc: use_signal(|| Doc::Current),
            },
            platform,
            pane_chrome: PaneChrome::default(),
        }
    }

    /// The document on screen: the full open when it is in, otherwise the first frame.
    pub(super) fn shown(&self) -> Option<(Ticket, LoadedDoc)> {
        self.loaded
            .read()
            .clone()
            .or_else(|| self.peeked.read().clone())
    }

    /// The same without subscribing the caller to a change.
    pub(super) fn shown_now(&self) -> Option<(Ticket, LoadedDoc)> {
        self.loaded
            .peek()
            .clone()
            .or_else(|| self.peeked.peek().clone())
    }
}

/// Sends an input to the root machine (which reads its context, from the window, at each step).
#[derive(Clone, Copy)]
pub(super) struct Dispatch {
    pub machine: MachineRef<Viewer>,
    pub shelf: Shelf,
    pub area: Memo<Option<Area>>,
}

/// Everything the root machine reads besides its inputs, from what the window knows now: its
/// context, read at every step.
pub(super) fn viewer_params(
    state: &Viewer,
    shelf: Shelf,
    area: Memo<Option<Area>>,
) -> ViewerParams {
    let doc = shelf.shown_now().map(|(_, doc)| doc);
    let lines = shelf.lines.peek().clone();
    params(
        &state.stage,
        doc.as_ref(),
        &shelf.probe.peek(),
        *area.peek(),
        &state.palette,
        lines.as_ref().map(|held| held.0.as_ref()),
        Live {
            level: shelf.level.now(),
            abilities: shelf.media.peek().abilities,
            platform: shelf.platform,
            tool: state.hand.tool,
            marks: shelf.media.peek().marks,
            picture: if state.presentation == Presentation::Pane {
                PictureOffer::Unavailable
            } else if state.picture.is_edited() {
                PictureOffer::Edited
            } else if state.picture.can_edit() {
                PictureOffer::Pristine
            } else {
                PictureOffer::Unavailable
            },
            adjust: state.picture.adjust(),
            wraps: *shelf.wraps.peek(),
            presentation: state.presentation,
            chrome: shelf.pane_chrome,
        },
    )
}

impl Dispatch {
    /// What the machine reads right now (the context its next step gets).
    pub(super) fn params(&self) -> ViewerParams {
        viewer_params(&self.machine.state().peek(), self.shelf, self.area)
    }

    pub(super) fn send(&self, input: ViewerIn) {
        self.machine.send(input);
    }
}

/// The room the content has, measured, in the window's scale.
pub(super) fn use_area(scale: Scale) -> (Memo<Option<Area>>, ds::host::measure::RectProbe) {
    let measured = use_rect();
    let area = use_memo(move || {
        measured.rect().map(|rect| Area {
            origin: rect.origin,
            size: rect.size,
            scale: scale.0 as f32 / Scale::DENOMINATOR as f32,
        })
    });
    (area, measured)
}
