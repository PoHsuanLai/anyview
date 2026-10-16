//! The trait each family implements once: how a stage draws, finds its controls and fills the
//! side panel.

use super::cx::{Area, HitLine, Leaving, StageCx};
use super::shared::EditDone;
use crate::io::{Job, MediaLine, NaturalSize, OpenError, OpenPort};
use crate::{
    Command, EditOffer, HitIndex, LoadFlow, MediaOffer, PanelTab, PanelTabs, Stage, StageIn,
    StageParams, Ticket, TypedText,
};
use anyview_core::work::Stop;
use anyview_core::{Facts, FindOptions, LineIndex, Resume, Sniffed, Source};
use anyview_text::EditStamp;
use dioxus::prelude::*;
use ds_core::selection::RowSelection;
use std::fmt::Debug;
use std::sync::Arc;

/// One family of formats in the viewer's full tier.
pub trait StageView: 'static {
    /// The family of stage machine this draws.
    const FAMILY: crate::StageFamily;
    /// Whether a file of this family has a first frame cheaper than the full open.
    const FLOW: LoadFlow = LoadFlow::OpenOnly;
    /// What happens to the document when the person leaves its file.
    const LEAVING: Leaving = Leaving::Keep;
    /// What opening a file makes: the decoded picture, the text index.
    type Doc: Debug + Send + Sync + 'static;

    /// Open the file whose type `sniffed` established. Blocking: runs on a worker, and uploads
    /// what it can into `link` itself. `stop` is raised when the person has left the file: an open
    /// asks it inside the loops that are its own (indexing, unpacking, laying out) and ends with an
    /// error nobody reads. A decoder that takes no `Stop` is not reached by it.
    fn open(
        ticket: Ticket,
        src: &Source,
        sniffed: &Sniffed,
        link: &OpenPort,
        stop: &Stop,
    ) -> Result<Self::Doc, OpenError>;
    /// The cheap first frame of the file, shown while `open` runs, or `None` when there is not one
    /// for this file. Blocking, on a worker; only called for a family whose `FLOW` is
    /// `PeekThenOpen`.
    fn first_frame(
        _ticket: Ticket,
        _src: &Source,
        _sniffed: &Sniffed,
        _link: &OpenPort,
        _stop: &Stop,
    ) -> Result<Option<Self::Doc>, OpenError> {
        Ok(None)
    }
    /// The rows the Info tab lists.
    fn facts(doc: &Self::Doc) -> Facts;
    /// The tabs the panel has for this document.
    fn tabs(doc: &Self::Doc) -> PanelTabs;
    /// What the stage machines are told of the document and the room: the scale the content is
    /// shown at, which views a text file has.
    fn params(doc: &Self::Doc, stage: &Stage, area: Option<Area>) -> StageParams;
    /// `params` for a family whose parameters depend on the lines last read (how many of them fit
    /// a page once long lines wrap); the others need not say.
    fn params_seen(
        doc: &Self::Doc,
        stage: &Stage,
        area: Option<Area>,
        _lines: Option<&super::super::LineWindow>,
    ) -> StageParams {
        Self::params(doc, stage, area)
    }
    /// The inputs the stage is told when `doc` lands, given the stage that is showing and where
    /// the file was left (`Opened::resume`): an animation says it moves, a find already up is
    /// asked again of the new copy, a place that needs the document's extent is put back.
    fn arrived(_doc: &Self::Doc, _stage: &Stage, _left_at: &Resume) -> Vec<StageIn> {
        Vec::new()
    }
    /// How big the content naturally is, once loaded, when only the loaded document knows (a
    /// PDF's first page at 100%, a picture a plugin decoded): what the window is sized to after
    /// its first file has loaded.
    fn natural(_doc: &Self::Doc) -> Option<NaturalSize> {
        None
    }
    /// The content.
    fn stage(doc: &Arc<Self::Doc>, cx: &StageCx) -> Element;
    /// The capsule's controls, left to right.
    fn slots(
        doc: &Self::Doc,
        cx: &StageCx,
    ) -> Vec<ds::components::chrome::capsule::priority::RankedSlot<Command>>;
    /// The panel's body for `tab`, when this family has something for it beyond the facts.
    fn panel(doc: &Arc<Self::Doc>, tab: PanelTab, cx: &StageCx) -> Option<Element>;
    /// The controls on the titlebar's trailing side, for a family with a mode to choose (a
    /// picture's Select | Pan). The titlebar fades and returns with the capsule.
    fn modes(_doc: &Arc<Self::Doc>, _cx: &StageCx) -> Option<Element> {
        None
    }
    /// The job that reads `rows` lines from `first`, for a family that shows lines.
    fn lines(_doc: &Arc<Self::Doc>, _ticket: Ticket, _first: LineIndex, _rows: u32) -> Option<Job> {
        None
    }
    /// The job that reads the document's file whole to edit it in place, for a family whose
    /// files are edited as text.
    fn edit_read(_doc: &Arc<Self::Doc>, _ticket: Ticket) -> Option<Job> {
        None
    }
    /// The job that finds `query` in the document as `options` say, for a family that can search.
    /// `stage` is what the stage shows now (the sheet of a table, the view of a text): a search
    /// is of what is on screen.
    fn search(
        _doc: &Arc<Self::Doc>,
        _stage: &Stage,
        _ticket: Ticket,
        _query: &TypedText,
        _options: FindOptions,
    ) -> Option<Job> {
        None
    }
    /// The inputs that show the hit numbered `hit` of the find that is up, for a family whose
    /// hits are not lines the window scrolls to (a table's cell, a tree's node): the cursor goes
    /// to it, and the nodes above it open. `sorted` is the order the rows of the table are shown
    /// in, when they are sorted.
    fn show_hit(
        _doc: &Arc<Self::Doc>,
        _stage: &Stage,
        _hits: &super::super::FoundHits,
        _sorted: Option<&super::super::SortedOrder>,
        _hit: HitIndex,
    ) -> Vec<StageIn> {
        Vec::new()
    }
    /// The text Copy puts on the clipboard for `rows`, the places the stage has picked, for a
    /// family whose rows are picked (a table's). `sorted` is the order the rows are shown in,
    /// when they are sorted.
    fn copy_text(
        _doc: &Arc<Self::Doc>,
        _stage: &Stage,
        _rows: &RowSelection,
        _sorted: Option<&super::super::SortedOrder>,
    ) -> Option<String> {
        None
    }
    /// What carrying out the edits `out` asks of the document comes to, for a family whose files
    /// are edited in place: the stage is told what the edits stand at, and a save says what is to
    /// be written. `order` is the order the rows are shown in, when they are sorted.
    fn edit(
        _doc: &Arc<Self::Doc>,
        _ticket: Ticket,
        _stage: &Stage,
        _out: &crate::StageOut,
        _order: Option<&super::super::SortedOrder>,
    ) -> EditDone {
        EditDone::default()
    }
    /// The edits as the stage is to hear them once the file holds the state of `stamp`.
    fn written(_doc: &Self::Doc, _stamp: EditStamp) -> Vec<StageIn> {
        Vec::new()
    }
    /// The first `upto` hits of the find that is up, as the palette lists them, for a family that
    /// can search.
    fn hit_lines(_doc: &Arc<Self::Doc>, _cx: &StageCx, _upto: u32) -> Vec<HitLine> {
        Vec::new()
    }
    /// The line to the player, for a family that plays.
    fn line(_doc: &Self::Doc) -> Option<Arc<dyn MediaLine>> {
        None
    }
    /// The media exports on offer, for a family that has them.
    fn media_offer(_doc: &Self::Doc) -> MediaOffer {
        MediaOffer::default()
    }
    /// What an edit of the document costs, for a family whose files are edited in place.
    fn edit_offer(_doc: &Self::Doc) -> EditOffer {
        EditOffer::Plain
    }
    /// Where the document's fields are and what it lets the person write on it, for a family
    /// whose files are filled and marked.
    fn markup(_doc: &Self::Doc) -> Option<crate::MarkupIn> {
        None
    }
    /// The tool the document is shown without because the system lacks it, when installing it
    /// would let the file show fully.
    fn lacks(_doc: &Self::Doc) -> Option<anyview_core::Helper> {
        None
    }
}
