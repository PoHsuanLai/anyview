//! Turning what the person asked for into an input for the stage that is showing: palette
//! commands and keys both end here, so a command means one thing wherever it came from.

use super::data_edits::{EditSessionIn, TypingIn, TypingSeed, TypingState};
use super::find::{FindHits, HitIndex, StageFind};
use super::media::{MediaIn, MediaStage};
use super::model::{Stage, StageIn, StageParams};
use super::pdf::{PdfIn, PdfStage};
use super::raster::RasterStage;
use super::table::{TableIn, TableStage};
use super::text::{Changes, Editable, Edited, TextIn, TextStage};
use super::tree::{TreeIn, TreeStage};
use super::{media, pdf, raster, table, text, tree};
use crate::command::StageCommand;
use crate::edits::Rewind;
use crate::typed::TypedText;

impl Stage {
    /// The input that carries out `command` on this stage, or `None` when this stage has no
    /// such command (a PDF has no source view, an image no playback to toggle).
    #[must_use]
    pub fn input_for(&self, command: StageCommand, params: &StageParams) -> Option<StageIn> {
        match self {
            Stage::NoStage => None,
            Stage::Raster(stage) => {
                raster::input_for(command, stage, &params.raster).map(StageIn::Raster)
            }
            Stage::Pdf(stage) => pdf::input_for(command, stage, &params.pdf).map(StageIn::Pdf),
            Stage::Media(_) => {
                media::input_for(command, &params.media.abilities).map(StageIn::Media)
            }
            Stage::Text(stage) => text::input_for(command, stage, &params.text).map(StageIn::Text),
            Stage::Table(stage) => {
                table::input_for(command, stage, &params.table).map(StageIn::Table)
            }
            Stage::Tree(stage) => tree::input_for(command, stage, &params.tree).map(StageIn::Tree),
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
