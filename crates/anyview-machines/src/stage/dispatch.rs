//! Turning what the person asked for into an input for the stage that is showing: palette
//! commands and keys both end here, so a command means one thing wherever it came from.

use super::data_edits::{EditSessionIn, TypingIn, TypingSeed, TypingState};
use super::find::{FindHits, HitIndex, StageFind};
use super::media::{
    ControlOffer, MediaAbilities, MediaIn, MediaStage, StepDirection, TrackKind, TrimEdge,
};
use super::model::{Stage, StageIn, StageParams};
use super::pdf::{LineDir, PageEdits, PdfIn, PdfParams, PdfStage, end, nudged, start};
use super::raster::{RasterIn, RasterParams, RasterStage};
use super::row::{RowNo, RowStep};
use super::table::{CellStep, TableIn, TableParams, TableStage};
use super::text::{Changes, Editable, Edited, TextIn, TextParams, TextStage, TextStep};
use super::tree::{KeyUnder, NodeUnder, TreeIn, TreeParams, TreeStage};
use super::zoom::ZoomDir;
use crate::command::StageCommand;
use crate::edits::Rewind;
use crate::typed::TypedText;
use anyview_core::{Speed, Zoom};

impl Stage {
    /// The input that carries out `command` on this stage, or `None` when this stage has no
    /// such command (a PDF has no source view, an image no playback to toggle).
    #[must_use]
    pub fn input_for(&self, command: StageCommand, params: &StageParams) -> Option<StageIn> {
        match self {
            Stage::NoStage => None,
            Stage::Raster(stage) => raster(command, stage, &params.raster).map(StageIn::Raster),
            Stage::Pdf(stage) => pdf(command, stage, &params.pdf).map(StageIn::Pdf),
            Stage::Media(_) => media(command, &params.media.abilities).map(StageIn::Media),
            Stage::Text(stage) => text(command, stage, &params.text).map(StageIn::Text),
            Stage::Table(stage) => table(command, stage, &params.table).map(StageIn::Table),
            Stage::Tree(stage) => tree(command, stage, &params.tree).map(StageIn::Tree),
        }
    }

    /// What has been done to the text, while the stage is editing it.
    #[must_use]
    pub fn edited(&self) -> Option<Edited> {
        match self {
            Stage::Text(stage) => stage.edited(),
            Stage::NoStage
            | Stage::Raster(_)
            | Stage::Pdf(_)
            | Stage::Media(_)
            | Stage::Table(_)
            | Stage::Tree(_) => None,
        }
    }

    /// Whether the data the stage edits (a table's cells, a tree's nodes) has changes that are not
    /// on disk. A text's are told by [`Stage::edited`].
    #[must_use]
    pub fn data_changes(&self) -> Changes {
        match self {
            Stage::Table(stage) => stage.cells().edits.log.changes,
            Stage::Tree(stage) => stage.edits().log.changes,
            Stage::NoStage
            | Stage::Raster(_)
            | Stage::Pdf(_)
            | Stage::Media(_)
            | Stage::Text(_) => Changes::Saved,
        }
    }

    /// The input that takes back the last edit of a table's cells or a tree's nodes, or does the
    /// one taken back again, when there is one to; `None` for the other stages and when nothing is
    /// held to go back to.
    #[must_use]
    pub fn rewind_input(&self, rewind: Rewind) -> Option<StageIn> {
        let log = match self {
            Stage::Table(stage) => stage.cells().edits.log,
            Stage::Tree(stage) => stage.edits().log,
            Stage::NoStage
            | Stage::Raster(_)
            | Stage::Pdf(_)
            | Stage::Media(_)
            | Stage::Text(_) => return None,
        };
        let (held, input) = match rewind {
            Rewind::Undo => (log.undo, EditSessionIn::Undo),
            Rewind::Redo => (log.redo, EditSessionIn::Redo),
        };
        if held.0 == 0 {
            return None;
        }
        match self {
            Stage::Table(_) => Some(StageIn::Table(TableIn::Session(input))),
            Stage::Tree(_) => Some(StageIn::Tree(TreeIn::Session(input))),
            Stage::NoStage
            | Stage::Raster(_)
            | Stage::Pdf(_)
            | Stage::Media(_)
            | Stage::Text(_) => None,
        }
    }

    /// The input that starts typing `text` into the cell under a table's cell cursor, when a key
    /// typed now would: the cursor is on a cell of a file that can be edited and nothing is typed
    /// in. `None` anywhere else, so the key is a key of the viewer's.
    #[must_use]
    pub fn typed_into(&self, text: TypedText, params: &StageParams) -> Option<StageIn> {
        match self {
            Stage::Table(stage)
                if stage.column().is_some()
                    && stage.cells().edits.typing == TypingState::Idle
                    && params.table.editable == Editable::Yes =>
            {
                Some(StageIn::Table(TableIn::Typing(TypingIn::Begin(
                    TypingSeed::Typed(text),
                ))))
            }
            Stage::Table(_)
            | Stage::NoStage
            | Stage::Raster(_)
            | Stage::Pdf(_)
            | Stage::Media(_)
            | Stage::Text(_)
            | Stage::Tree(_) => None,
        }
    }

    /// Whether a value of a table or a tree is being typed into: the keys are the field's.
    #[must_use]
    pub fn is_typing(&self) -> bool {
        match self {
            Stage::Table(stage) => stage.cells().edits.typing != TypingState::Idle,
            Stage::Tree(stage) => stage.edits().typing != TypingState::Idle,
            Stage::NoStage
            | Stage::Raster(_)
            | Stage::Pdf(_)
            | Stage::Media(_)
            | Stage::Text(_) => false,
        }
    }

    /// Whether the person has a row picked in a table or a tree: Esc puts it away, and until then
    /// the keys that would walk the folder are the reader's.
    #[must_use]
    pub fn has_cursor(&self) -> bool {
        match self {
            Stage::Table(TableStage::Selected { .. }) | Stage::Tree(TreeStage::Selected { .. }) => {
                true
            }
            Stage::NoStage
            | Stage::Raster(_)
            | Stage::Pdf(_)
            | Stage::Media(_)
            | Stage::Text(_)
            | Stage::Table(TableStage::Browsing { .. })
            | Stage::Tree(TreeStage::Browsing { .. }) => false,
        }
    }

    /// The input that searches the file for `query`, for a stage that can search.
    #[must_use]
    pub fn find_input(&self, query: &TypedText) -> Option<StageIn> {
        match self {
            Stage::Text(_) => Some(StageIn::Text(TextIn::Find(query.clone()))),
            Stage::Pdf(_) => Some(StageIn::Pdf(PdfIn::Find(query.clone()))),
            Stage::Table(_) => Some(StageIn::Table(TableIn::Find(query.clone()))),
            Stage::Tree(_) => Some(StageIn::Tree(TreeIn::Find(query.clone()))),
            Stage::NoStage | Stage::Raster(_) | Stage::Media(_) => None,
        }
    }

    /// The input that makes `hit` the current one of the find that is up.
    #[must_use]
    pub fn hit_input(&self, hit: HitIndex) -> Option<StageIn> {
        match self {
            Stage::Text(TextStage::Finding { .. } | TextStage::Editing { find: Some(_), .. }) => {
                Some(StageIn::Text(TextIn::GoToHit(hit)))
            }
            Stage::Pdf(PdfStage::Finding { .. }) => Some(StageIn::Pdf(PdfIn::GoToHit(hit))),
            Stage::Table(stage) if stage.find().is_some() => {
                Some(StageIn::Table(TableIn::GoToHit(hit)))
            }
            Stage::Tree(stage) if stage.find().is_some() => {
                Some(StageIn::Tree(TreeIn::GoToHit(hit)))
            }
            Stage::NoStage
            | Stage::Raster(_)
            | Stage::Media(_)
            | Stage::Pdf(PdfStage::Reading { .. } | PdfStage::Jumping { .. })
            | Stage::Text(TextStage::Reading { .. } | TextStage::Editing { find: None, .. })
            | Stage::Table(_)
            | Stage::Tree(_) => None,
        }
    }

    /// The text of the find that is up and where its search stands.
    #[must_use]
    pub fn find_state(&self) -> Option<(&TypedText, FindHits)> {
        match self {
            Stage::Pdf(PdfStage::Finding { query, hits, .. })
            | Stage::Text(
                TextStage::Finding { query, hits, .. }
                | TextStage::Editing {
                    find: Some(StageFind { query, hits }),
                    ..
                },
            ) => Some((query, *hits)),
            Stage::Table(stage) => stage.find().map(|find| (&find.query, find.hits)),
            Stage::Tree(stage) => stage.find().map(|find| (&find.query, find.hits)),
            Stage::NoStage
            | Stage::Raster(_)
            | Stage::Media(_)
            | Stage::Pdf(PdfStage::Reading { .. } | PdfStage::Jumping { .. })
            | Stage::Text(TextStage::Reading { .. } | TextStage::Editing { find: None, .. }) => {
                None
            }
        }
    }

    /// Whether a find is up: its hits are marked, and Esc puts it away.
    #[must_use]
    pub fn is_finding(&self) -> bool {
        match self {
            Stage::Pdf(PdfStage::Finding { .. })
            | Stage::Text(TextStage::Finding { .. } | TextStage::Editing { find: Some(_), .. }) => {
                true
            }
            Stage::Table(stage) => stage.find().is_some(),
            Stage::Tree(stage) => stage.find().is_some(),
            Stage::NoStage
            | Stage::Raster(_)
            | Stage::Media(_)
            | Stage::Pdf(PdfStage::Reading { .. } | PdfStage::Jumping { .. })
            | Stage::Text(TextStage::Reading { .. } | TextStage::Editing { find: None, .. }) => {
                false
            }
        }
    }

    /// The input that closes what the stage has open on top of its content (a find bar, a
    /// scrub), or `None` when it has nothing open: Esc undoes this before anything else.
    #[must_use]
    pub fn dismissal(&self) -> Option<StageIn> {
        match self {
            Stage::NoStage => None,
            Stage::Raster(
                RasterStage::Fitted { .. }
                | RasterStage::Zoomed { .. }
                | RasterStage::Panning { .. },
            ) => None,
            Stage::Pdf(PdfStage::Finding { .. }) => Some(StageIn::Pdf(PdfIn::CloseFind)),
            Stage::Pdf(PdfStage::Reading { .. } | PdfStage::Jumping { .. }) => None,
            Stage::Media(MediaStage::Scrubbing { .. }) => {
                Some(StageIn::Media(MediaIn::ScrubCancel))
            }
            Stage::Media(
                MediaStage::Opening
                | MediaStage::Playing { .. }
                | MediaStage::Paused { .. }
                | MediaStage::Ended { .. }
                | MediaStage::Failed(_),
            ) => None,
            Stage::Text(TextStage::Finding { .. } | TextStage::Editing { find: Some(_), .. }) => {
                Some(StageIn::Text(TextIn::CloseFind))
            }
            Stage::Text(TextStage::Reading { .. } | TextStage::Editing { find: None, .. }) => None,
            // A find is the innermost thing a table or a tree has open; the cursor is next.
            Stage::Table(stage) => match (stage.find(), stage.row()) {
                (Some(_), _) => Some(StageIn::Table(TableIn::CloseFind)),
                (None, Some(_)) => Some(StageIn::Table(TableIn::Deselect)),
                (None, None) => None,
            },
            Stage::Tree(stage) => match (stage.find(), stage.row()) {
                (Some(_), _) => Some(StageIn::Tree(TreeIn::CloseFind)),
                (None, Some(_)) => Some(StageIn::Tree(TreeIn::Deselect)),
                (None, None) => None,
            },
        }
    }
}

fn raster(command: StageCommand, stage: &RasterStage, params: &RasterParams) -> Option<RasterIn> {
    let at = params.centre;
    match command {
        StageCommand::ZoomIn => Some(RasterIn::ZoomStep {
            dir: ZoomDir::In,
            at,
        }),
        StageCommand::ZoomOut => Some(RasterIn::ZoomStep {
            dir: ZoomDir::Out,
            at,
        }),
        StageCommand::ZoomToFit => Some(RasterIn::SetZoom {
            zoom: Zoom::Fit,
            at,
        }),
        StageCommand::ZoomToActual => Some(RasterIn::SetZoom {
            zoom: Zoom::Actual,
            at,
        }),
        StageCommand::TogglePlayback if stage.is_animated() => Some(RasterIn::TogglePlayback),
        StageCommand::StepFrameForward if stage.is_animated() => {
            Some(RasterIn::StepFrame(StepDirection::Forward))
        }
        StageCommand::StepFrameBack if stage.is_animated() => {
            Some(RasterIn::StepFrame(StepDirection::Backward))
        }
        StageCommand::TogglePlayback
        | StageCommand::StepFrameForward
        | StageCommand::StepFrameBack
        | StageCommand::ZoomToWidth
        | StageCommand::Find
        | StageCommand::FindNext
        | StageCommand::FindPrevious
        | StageCommand::ToggleSource
        | StageCommand::ToggleWrap
        | StageCommand::SeekBack
        | StageCommand::SeekForward
        | StageCommand::NextPage
        | StageCommand::PreviousPage
        | StageCommand::LineUp
        | StageCommand::LineDown
        | StageCommand::ScrollToStart
        | StageCommand::ScrollToEnd
        | StageCommand::SlowDown
        | StageCommand::SpeedUp
        | StageCommand::NormalSpeed
        | StageCommand::NextChapter
        | StageCommand::PreviousChapter
        | StageCommand::NextAudioTrack
        | StageCommand::NextSubtitles
        | StageCommand::MarkTrimStart
        | StageCommand::MarkTrimEnd
        | StageCommand::DeletePage
        | StageCommand::MovePageEarlier
        | StageCommand::MovePageLater
        | StageCommand::NextSheet
        | StageCommand::PreviousSheet
        | StageCommand::CollapseAll
        | StageCommand::SelectAll
        | StageCommand::CopyRows
        | StageCommand::Edit
        | StageCommand::Done
        | StageCommand::Save
        | StageCommand::PreviousCell
        | StageCommand::NextCell
        | StageCommand::AddRow
        | StageCommand::DeletePicked
        | StageCommand::AddItem
        | StageCommand::RenameKey => None,
    }
}

fn pdf(command: StageCommand, stage: &PdfStage, params: &PdfParams) -> Option<PdfIn> {
    match command {
        StageCommand::ZoomIn => Some(PdfIn::ZoomStep(ZoomDir::In)),
        StageCommand::ZoomOut => Some(PdfIn::ZoomStep(ZoomDir::Out)),
        StageCommand::ZoomToFit => Some(PdfIn::SetZoom(Zoom::Fit)),
        StageCommand::ZoomToWidth => Some(PdfIn::SetZoom(Zoom::Fill)),
        StageCommand::ZoomToActual => Some(PdfIn::SetZoom(Zoom::Actual)),
        StageCommand::Find => Some(PdfIn::Find(TypedText::EMPTY)),
        StageCommand::FindNext => Some(PdfIn::NextHit),
        StageCommand::FindPrevious => Some(PdfIn::PreviousHit),
        StageCommand::NextPage => Some(PdfIn::NextPage),
        StageCommand::PreviousPage => Some(PdfIn::PreviousPage),
        StageCommand::LineUp => Some(PdfIn::GoTo(nudged(
            stage.place(),
            LineDir::Up,
            params.pages,
        ))),
        StageCommand::LineDown => Some(PdfIn::GoTo(nudged(
            stage.place(),
            LineDir::Down,
            params.pages,
        ))),
        StageCommand::ScrollToStart => Some(PdfIn::GoTo(start())),
        StageCommand::ScrollToEnd => Some(PdfIn::GoTo(end(params.pages))),
        StageCommand::DeletePage if params.page_edits == PageEdits::Allowed => {
            Some(PdfIn::DeletePage)
        }
        StageCommand::MovePageEarlier if params.page_edits == PageEdits::Allowed => {
            Some(PdfIn::MovePage(StepDirection::Backward))
        }
        StageCommand::MovePageLater if params.page_edits == PageEdits::Allowed => {
            Some(PdfIn::MovePage(StepDirection::Forward))
        }
        StageCommand::ToggleSource
        | StageCommand::ToggleWrap
        | StageCommand::TogglePlayback
        | StageCommand::SeekBack
        | StageCommand::SeekForward
        | StageCommand::SlowDown
        | StageCommand::SpeedUp
        | StageCommand::NormalSpeed
        | StageCommand::NextChapter
        | StageCommand::PreviousChapter
        | StageCommand::NextAudioTrack
        | StageCommand::NextSubtitles
        | StageCommand::StepFrameForward
        | StageCommand::StepFrameBack
        | StageCommand::MarkTrimStart
        | StageCommand::MarkTrimEnd
        | StageCommand::NextSheet
        | StageCommand::PreviousSheet
        | StageCommand::DeletePage
        | StageCommand::MovePageEarlier
        | StageCommand::MovePageLater
        | StageCommand::CollapseAll
        | StageCommand::SelectAll
        | StageCommand::CopyRows
        | StageCommand::Edit
        | StageCommand::Done
        | StageCommand::Save
        | StageCommand::PreviousCell
        | StageCommand::NextCell
        | StageCommand::AddRow
        | StageCommand::DeletePicked
        | StageCommand::AddItem
        | StageCommand::RenameKey => None,
    }
}

/// The input a media command is, or `None` when the player cannot do it.
fn media(command: StageCommand, abilities: &MediaAbilities) -> Option<MediaIn> {
    let needs = |control: ControlOffer, input: MediaIn| match control {
        ControlOffer::Offered => Some(input),
        ControlOffer::Withheld => None,
    };
    match command {
        StageCommand::TogglePlayback => Some(MediaIn::Toggle),
        StageCommand::SeekBack => Some(MediaIn::SeekBack),
        StageCommand::SeekForward => Some(MediaIn::SeekForward),
        StageCommand::SlowDown => {
            needs(abilities.speed, MediaIn::StepSpeed(StepDirection::Backward))
        }
        StageCommand::SpeedUp => needs(abilities.speed, MediaIn::StepSpeed(StepDirection::Forward)),
        StageCommand::NormalSpeed => needs(abilities.speed, MediaIn::SetSpeed(Speed::NORMAL)),
        StageCommand::NextChapter => needs(
            abilities.chapters,
            MediaIn::StepChapter(StepDirection::Forward),
        ),
        StageCommand::PreviousChapter => needs(
            abilities.chapters,
            MediaIn::StepChapter(StepDirection::Backward),
        ),
        StageCommand::NextAudioTrack => {
            needs(abilities.tracks, MediaIn::CycleTrack(TrackKind::Audio))
        }
        StageCommand::NextSubtitles => {
            needs(abilities.tracks, MediaIn::CycleTrack(TrackKind::Subtitles))
        }
        StageCommand::StepFrameForward => needs(
            abilities.frame_step,
            MediaIn::FrameStep(StepDirection::Forward),
        ),
        StageCommand::StepFrameBack => needs(
            abilities.frame_step,
            MediaIn::FrameStep(StepDirection::Backward),
        ),
        StageCommand::MarkTrimStart => Some(MediaIn::Mark(TrimEdge::Start)),
        StageCommand::MarkTrimEnd => Some(MediaIn::Mark(TrimEdge::End)),
        StageCommand::ZoomIn
        | StageCommand::ZoomOut
        | StageCommand::ZoomToFit
        | StageCommand::ZoomToWidth
        | StageCommand::ZoomToActual
        | StageCommand::Find
        | StageCommand::FindNext
        | StageCommand::FindPrevious
        | StageCommand::ToggleSource
        | StageCommand::ToggleWrap
        | StageCommand::NextPage
        | StageCommand::PreviousPage
        | StageCommand::LineUp
        | StageCommand::LineDown
        | StageCommand::ScrollToStart
        | StageCommand::ScrollToEnd
        | StageCommand::DeletePage
        | StageCommand::MovePageEarlier
        | StageCommand::MovePageLater
        | StageCommand::NextSheet
        | StageCommand::PreviousSheet
        | StageCommand::CollapseAll
        | StageCommand::SelectAll
        | StageCommand::CopyRows
        | StageCommand::Edit
        | StageCommand::Done
        | StageCommand::Save
        | StageCommand::PreviousCell
        | StageCommand::NextCell
        | StageCommand::AddRow
        | StageCommand::DeletePicked
        | StageCommand::AddItem
        | StageCommand::RenameKey => None,
    }
}

fn text(command: StageCommand, stage: &TextStage, params: &TextParams) -> Option<TextIn> {
    let editing = stage.edited().is_some();
    match command {
        StageCommand::Find => Some(TextIn::Find(TypedText::EMPTY)),
        StageCommand::FindNext => Some(TextIn::NextHit),
        StageCommand::FindPrevious => Some(TextIn::PreviousHit),
        StageCommand::ToggleSource if !editing => Some(TextIn::ToggleSource),
        StageCommand::ToggleWrap => Some(TextIn::ToggleWrap),
        StageCommand::ZoomIn => Some(TextIn::ZoomStep(ZoomDir::In)),
        StageCommand::ZoomOut => Some(TextIn::ZoomStep(ZoomDir::Out)),
        StageCommand::ZoomToActual => Some(TextIn::ZoomActual),
        StageCommand::Edit if !editing && params.editable == Editable::Yes => Some(TextIn::Edit),
        StageCommand::Done if editing => Some(TextIn::Done),
        StageCommand::Save if editing => Some(TextIn::Save),
        StageCommand::NextPage => Some(TextIn::Step(TextStep::PageDown)),
        StageCommand::PreviousPage => Some(TextIn::Step(TextStep::PageUp)),
        StageCommand::LineUp => Some(TextIn::Step(TextStep::LineUp)),
        StageCommand::LineDown => Some(TextIn::Step(TextStep::LineDown)),
        StageCommand::ScrollToStart => Some(TextIn::Step(TextStep::Top)),
        StageCommand::ScrollToEnd => Some(TextIn::Step(TextStep::Bottom)),
        StageCommand::ToggleSource
        | StageCommand::Edit
        | StageCommand::Done
        | StageCommand::Save
        | StageCommand::ZoomToFit
        | StageCommand::ZoomToWidth
        | StageCommand::TogglePlayback
        | StageCommand::SeekBack
        | StageCommand::SeekForward
        | StageCommand::SlowDown
        | StageCommand::SpeedUp
        | StageCommand::NormalSpeed
        | StageCommand::NextChapter
        | StageCommand::PreviousChapter
        | StageCommand::NextAudioTrack
        | StageCommand::NextSubtitles
        | StageCommand::StepFrameForward
        | StageCommand::StepFrameBack
        | StageCommand::MarkTrimStart
        | StageCommand::MarkTrimEnd
        | StageCommand::DeletePage
        | StageCommand::MovePageEarlier
        | StageCommand::MovePageLater
        | StageCommand::NextSheet
        | StageCommand::PreviousSheet
        | StageCommand::CollapseAll
        | StageCommand::SelectAll
        | StageCommand::CopyRows
        | StageCommand::PreviousCell
        | StageCommand::NextCell
        | StageCommand::AddRow
        | StageCommand::DeletePicked
        | StageCommand::AddItem
        | StageCommand::RenameKey => None,
    }
}

fn table(command: StageCommand, stage: &TableStage, params: &TableParams) -> Option<TableIn> {
    let editing = params.editable == Editable::Yes;
    let picked = stage.row().is_some();
    match command {
        StageCommand::NextSheet => Some(TableIn::NextSheet),
        StageCommand::PreviousSheet => Some(TableIn::PreviousSheet),
        StageCommand::LineUp => Some(TableIn::Move(RowStep::Up)),
        StageCommand::LineDown => Some(TableIn::Move(RowStep::Down)),
        StageCommand::PreviousPage => Some(TableIn::Move(RowStep::PageUp)),
        StageCommand::NextPage => Some(TableIn::Move(RowStep::PageDown)),
        StageCommand::ScrollToStart => Some(TableIn::Move(RowStep::Top)),
        StageCommand::ScrollToEnd => Some(TableIn::Move(RowStep::Bottom)),
        StageCommand::Find => Some(TableIn::Find(TypedText::EMPTY)),
        StageCommand::FindNext => Some(TableIn::NextHit),
        StageCommand::FindPrevious => Some(TableIn::PreviousHit),
        StageCommand::ZoomIn => Some(TableIn::ZoomStep(ZoomDir::In)),
        StageCommand::ZoomOut => Some(TableIn::ZoomStep(ZoomDir::Out)),
        StageCommand::ZoomToActual => Some(TableIn::ZoomActual),
        StageCommand::SelectAll => Some(TableIn::SelectAll),
        StageCommand::CopyRows if stage.selected_count() > 0 => Some(TableIn::Copy),
        StageCommand::Edit if editing && picked => {
            Some(TableIn::Typing(TypingIn::Begin(TypingSeed::Existing)))
        }
        StageCommand::PreviousCell if editing && picked => Some(TableIn::StepCell(CellStep::Left)),
        StageCommand::NextCell if editing && picked => Some(TableIn::StepCell(CellStep::Right)),
        StageCommand::AddRow if editing && picked => Some(TableIn::AddRow),
        StageCommand::DeletePicked if editing && picked => Some(TableIn::DeleteRows),
        StageCommand::Save if stage.cells().edits.log.changes == Changes::Unsaved => {
            Some(TableIn::Session(EditSessionIn::Save))
        }
        StageCommand::ZoomToFit
        | StageCommand::ZoomToWidth
        | StageCommand::ToggleSource
        | StageCommand::ToggleWrap
        | StageCommand::TogglePlayback
        | StageCommand::SeekBack
        | StageCommand::SeekForward
        | StageCommand::SlowDown
        | StageCommand::SpeedUp
        | StageCommand::NormalSpeed
        | StageCommand::NextChapter
        | StageCommand::PreviousChapter
        | StageCommand::NextAudioTrack
        | StageCommand::NextSubtitles
        | StageCommand::StepFrameForward
        | StageCommand::StepFrameBack
        | StageCommand::MarkTrimStart
        | StageCommand::MarkTrimEnd
        | StageCommand::DeletePage
        | StageCommand::MovePageEarlier
        | StageCommand::MovePageLater
        | StageCommand::CollapseAll
        | StageCommand::CopyRows
        | StageCommand::Edit
        | StageCommand::Done
        | StageCommand::Save
        | StageCommand::PreviousCell
        | StageCommand::NextCell
        | StageCommand::AddRow
        | StageCommand::DeletePicked
        | StageCommand::AddItem
        | StageCommand::RenameKey => None,
    }
}

fn tree(command: StageCommand, stage: &TreeStage, params: &TreeParams) -> Option<TreeIn> {
    let editing = params.editable == Editable::Yes;
    let picked = stage.row();
    match command {
        StageCommand::CollapseAll => Some(TreeIn::CollapseAll),
        StageCommand::LineUp => Some(TreeIn::Move(RowStep::Up)),
        StageCommand::LineDown => Some(TreeIn::Move(RowStep::Down)),
        StageCommand::PreviousPage => Some(TreeIn::Move(RowStep::PageUp)),
        StageCommand::NextPage => Some(TreeIn::Move(RowStep::PageDown)),
        StageCommand::ScrollToStart => Some(TreeIn::Move(RowStep::Top)),
        StageCommand::ScrollToEnd => Some(TreeIn::Move(RowStep::Bottom)),
        StageCommand::Find => Some(TreeIn::Find(TypedText::EMPTY)),
        StageCommand::FindNext => Some(TreeIn::NextHit),
        StageCommand::FindPrevious => Some(TreeIn::PreviousHit),
        StageCommand::ZoomIn => Some(TreeIn::ZoomStep(ZoomDir::In)),
        StageCommand::ZoomOut => Some(TreeIn::ZoomStep(ZoomDir::Out)),
        StageCommand::ZoomToActual => Some(TreeIn::ZoomActual),
        StageCommand::Edit if editing && picked.is_some() && params.under == NodeUnder::Scalar => {
            Some(TreeIn::Typing(TypingIn::Begin(TypingSeed::Existing)))
        }
        StageCommand::RenameKey
            if editing && picked.is_some() && params.key == KeyUnder::Present =>
        {
            Some(TreeIn::RenameKey)
        }
        StageCommand::AddItem if editing && picked.is_some() => Some(TreeIn::AddItem),
        StageCommand::DeletePicked if editing && picked.is_some_and(|row| row != RowNo(0)) => {
            Some(TreeIn::DeleteItem)
        }
        StageCommand::Save if stage.edits().log.changes == Changes::Unsaved => {
            Some(TreeIn::Session(EditSessionIn::Save))
        }
        StageCommand::ZoomToFit
        | StageCommand::ZoomToWidth
        | StageCommand::ToggleSource
        | StageCommand::ToggleWrap
        | StageCommand::TogglePlayback
        | StageCommand::SeekBack
        | StageCommand::SeekForward
        | StageCommand::SlowDown
        | StageCommand::SpeedUp
        | StageCommand::NormalSpeed
        | StageCommand::NextChapter
        | StageCommand::PreviousChapter
        | StageCommand::NextAudioTrack
        | StageCommand::NextSubtitles
        | StageCommand::StepFrameForward
        | StageCommand::StepFrameBack
        | StageCommand::MarkTrimStart
        | StageCommand::MarkTrimEnd
        | StageCommand::DeletePage
        | StageCommand::MovePageEarlier
        | StageCommand::MovePageLater
        | StageCommand::NextSheet
        | StageCommand::PreviousSheet
        | StageCommand::SelectAll
        | StageCommand::CopyRows
        | StageCommand::Edit
        | StageCommand::Done
        | StageCommand::Save
        | StageCommand::PreviousCell
        | StageCommand::NextCell
        | StageCommand::AddRow
        | StageCommand::DeletePicked
        | StageCommand::AddItem
        | StageCommand::RenameKey => None,
    }
}
