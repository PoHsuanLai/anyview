//! What a family's views read from the window and send back to it: the stage's context, its
//! area, and the small values that go with them.

use super::shared::Held;
use crate::families::media::MediaShelf;
use crate::families::pdf::PdfShelf;
use crate::io::{HostRequest, Job};
use crate::{Command, Stage, StageIn, Ticket};
use anyview_core::LineIndex;
use dioxus::prelude::*;
use ds::prelude::{Point, Size};
use std::fmt::Debug;

/// Thousandths of zoom a logical pixel of a wheel turn under Control makes.
pub(crate) const WHEEL_ZOOM: f32 = 5.0;
/// The room the content has, as the window measured it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Area {
    /// The content box's top-left corner in the window, in logical pixels.
    pub origin: Point,
    /// The content box's size, in logical pixels.
    pub size: Size,
    /// Device pixels per logical pixel.
    pub scale: f32,
}
/// What a family's views read from the window and send back to it. Everything is a value or a
/// callback, so a view decides nothing the machines decide.
#[derive(Debug, Clone, PartialEq)]
pub struct StageCx {
    /// The stage machine's state.
    pub stage: Stage,
    /// The pan tool: whether a drag on a picture pans it.
    pub hand: crate::Hand,
    /// What has been done to the open picture and not saved, and the crop rectangle being drawn.
    pub picture: crate::PictureEdits,
    /// An input for the picture's edits (the crop rectangle's grips and buttons).
    pub edit: EventHandler<crate::PictureEditIn>,
    /// The trim of the open recording: the bar's handles while it is up, and the stretch kept.
    pub trim: crate::MediaTrim,
    /// An input for the trim (the bar's handles and buttons).
    pub trim_in: EventHandler<crate::TrimIn>,
    /// What has been written on the open PDF and not saved: the fields filled and the marks made.
    pub markup: crate::PdfMarkup,
    /// An input for the markup (a field typed in, a mark drawn).
    pub mark: EventHandler<crate::MarkupIn>,
    /// The load this document belongs to.
    pub ticket: Ticket,
    /// The room, once measured.
    pub area: Option<Area>,
    /// An input for the stage machine.
    pub send: EventHandler<StageIn>,
    /// A command from a control the stage drew (the peek-only view's Show in Folder).
    pub run: EventHandler<Command>,
    /// The lines of a text file last read, newest window.
    pub lines: Option<Held<super::super::LineWindow>>,
    /// The order a worker last gave a sheet of the open table, for the view to swap to.
    pub sorted: Option<Held<super::super::SortedOrder>>,
    /// Ask for a window of lines: from this line, this many.
    pub ask_lines: EventHandler<(LineIndex, u32)>,
    /// The places the current find found, when one is up.
    pub hits: Option<Held<super::super::FoundHits>>,
    /// Hand a job to the workers; its answer comes back through the window's mailbox.
    pub work: EventHandler<Job>,
    /// Ask the host to do something the viewer cannot (open a web address).
    pub request: EventHandler<HostRequest>,
    /// What the window holds of an open PDF: the tiles, the hits, the thumbnails.
    pub pdf: PdfShelf,
    /// What the window holds of the recording that plays: the position, the lists.
    pub media: MediaShelf,
    /// The scheme the content is drawn in, for a sealed frame that cannot inherit it.
    pub frame: FrameLook,
    /// What the platform can do: a control of a service it lacks is not drawn.
    pub platform: crate::PlatformAbilities,
    /// Whether the open file takes a save in place: a file that refuses one is not offered for
    /// editing.
    pub access: crate::FileAccess,
    /// The text being edited, once it is read: the window holds it so that saving, the find and
    /// the title bar's edited dot all see the one text.
    pub text_edit: Signal<Option<anyview_text::Session>>,
    /// The handle on the edit surface, so the window can give it the keyboard back.
    pub text_editor: ds::edit::handle::EditHandle,
}
/// One place a find found, as the palette lists it under "In This File": the words around the match
/// with the match marked, and where it is.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HitLine {
    /// The words around the match (a line of text, a snippet).
    pub context: String,
    /// The part of `context` the find matched, as byte offsets into it.
    pub matched: std::ops::Range<usize>,
    /// Where it is, as the person counts: `Line 42`, `Page 7`.
    pub place: String,
}
/// What a sealed frame needs to wear the window's look: the root's attributes, written into the
/// frame's own document because a frame inherits nothing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FrameLook {
    /// The `class` and `data-*` attributes of the window's `Ds` root, as `name="value"` pairs.
    pub attributes: String,
}
/// What becomes of a document when the person walks away from its file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leaving {
    /// It is kept as it was left, so coming back to the file needs no open.
    Keep,
    /// It is let go: a recording that kept its player would go on playing unseen.
    Release,
}
