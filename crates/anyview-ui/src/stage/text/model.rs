//! The text stage's states, inputs and outputs.

use super::super::find::{FindHits, FindOut, HitCount, HitIndex};
use crate::typed::TypedText;
use anyview_core::{LineIndex, Resume};
use ds_core::word::Word;

/// Whether long lines wrap at the window's edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word, Default)]
pub enum Wrap {
    /// Lines wrap.
    #[default]
    On,
    /// Lines run on and scroll sideways.
    Off,
}

/// What the text shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum TextView {
    /// Markdown as a rendered page.
    Rendered,
    /// The text as written, highlighted when it is code.
    Source,
}

/// Which views a file has: Markdown has both, everything else only its source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TextViews {
    /// Rendered and source.
    #[default]
    RenderedAndSource,
    /// Source only.
    SourceOnly,
}

/// Where the reader is in the text and how it is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextPlace {
    /// The line at the top of the view.
    pub line: LineIndex,
    /// Whether long lines wrap.
    pub wrap: Wrap,
    /// What is shown.
    pub view: TextView,
}

/// Whether the text being edited has changes that are not on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Changes {
    /// The text is what the file holds.
    #[default]
    Saved,
    /// The text has been changed since it was opened or last saved.
    Unsaved,
}

/// Whether the file was changed on disk, by another program, since it was opened or last saved
/// here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Outside {
    /// The file is as this window last knew it.
    #[default]
    Unchanged,
    /// Another program changed it.
    Changed,
}

/// What the person has done to the text, and what the disk has done to the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Edited {
    /// Whether the text has unsaved changes.
    pub changes: Changes,
    /// Whether the file changed under it.
    pub outside: Outside,
}

/// The find that is up while editing: its text, and where its search stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditFind {
    /// What is searched for.
    pub query: TypedText,
    /// Where the search stands.
    pub hits: FindHits,
}

/// What the text stage is doing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextStage {
    /// Reading.
    Reading { place: TextPlace },
    /// A find is up: `hits` says where its search stands.
    Finding {
        query: TypedText,
        hits: FindHits,
        place: TextPlace,
    },
    /// The source is being edited in place; a find may be up over it.
    Editing {
        place: TextPlace,
        edited: Edited,
        find: Option<EditFind>,
    },
}

impl TextStage {
    /// Where the reader is and how the text is shown.
    pub fn place(&self) -> TextPlace {
        match self {
            TextStage::Reading { place }
            | TextStage::Finding { place, .. }
            | TextStage::Editing { place, .. } => *place,
        }
    }

    /// What has been done to the text, while it is being edited.
    pub fn edited(&self) -> Option<Edited> {
        match self {
            TextStage::Editing { edited, .. } => Some(*edited),
            TextStage::Reading { .. } | TextStage::Finding { .. } => None,
        }
    }

    /// A stage at the top of a file with these views: rendered first when there is a rendering,
    /// wrapped.
    pub fn opened(views: TextViews) -> TextStage {
        let view = match views {
            TextViews::RenderedAndSource => TextView::Rendered,
            TextViews::SourceOnly => TextView::Source,
        };
        TextStage::Reading {
            place: TextPlace {
                line: LineIndex(0),
                wrap: Wrap::On,
                view,
            },
        }
    }
}

impl Default for TextStage {
    fn default() -> Self {
        TextStage::opened(TextViews::default())
    }
}

/// A step of the keyboard through the lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextStep {
    /// One line towards the start.
    LineUp,
    /// One line towards the end.
    LineDown,
    /// One page towards the start.
    PageUp,
    /// One page towards the end.
    PageDown,
    /// The first line.
    Top,
    /// The last page: the end of the file at the bottom of the view.
    Bottom,
}

/// What moves the stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextIn {
    /// The reader scrolled: this is the line at the top.
    Scroll(LineIndex),
    /// The reader asked for a step by key.
    Step(TextStep),
    /// Search for this text; empty text closes the find.
    Find(TypedText),
    /// The search for `query` found `count` hits, `nearest` being the one closest to the reader.
    Results {
        query: TypedText,
        count: HitCount,
        nearest: HitIndex,
    },
    /// The next hit, wrapping to the first after the last.
    NextHit,
    /// The previous hit, wrapping to the last before the first.
    PreviousHit,
    /// Make this hit the current one and show it (a row of the palette's hits).
    GoToHit(HitIndex),
    /// Close the find.
    CloseFind,
    /// Switch between rendered and source, when the file has both.
    ToggleSource,
    /// Switch line wrapping.
    ToggleWrap,
    /// Start editing the source in place.
    Edit,
    /// Stop editing and go back to reading.
    Done,
    /// Write the edited text to the file.
    Save,
    /// The edited text changed (or was undone or saved): this is whether it has unsaved changes
    /// now. Told after every change, so a find over the text searches it again.
    Edited(Changes),
    /// Whether the file differs on disk from what this window last knew of it.
    Disk(Outside),
    /// Restore where the person left the file.
    Restore(Resume),
    /// The clock; the stage keeps no timer.
    Elapsed,
}

impl From<ds_core::machine::Elapsed> for TextIn {
    fn from(_: ds_core::machine::Elapsed) -> Self {
        TextIn::Elapsed
    }
}

/// What the stage wants done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextOut {
    /// Keep where the person is, for next time.
    Remember(Resume),
    /// Scroll the view to this line.
    ScrollTo(LineIndex),
    /// Show the text this way.
    Show(TextView),
    /// Something for the search: run it, show a hit, clear the marks.
    Find(FindOut),
    /// Read the file whole and make its text editable.
    BeginEdit,
    /// Let go of the text being edited.
    EndEdit,
    /// Write the edited text to the file.
    Save,
    /// The person chose this wrapping: it is kept for the next file of the same kind.
    Wrapped(Wrap),
}

/// How many lines the open file has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LineTotal(pub u32);

/// How many lines fit the view at once: what a page step moves by. With wrapping on, a long line
/// takes several rows, so this is fewer than the rows the view has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PageLines(pub u32);

/// The size of the file and of a page of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextExtent {
    /// The lines the file has.
    pub lines: LineTotal,
    /// The lines in one page.
    pub page: PageLines,
}

/// Whether the open file can be edited in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Editable {
    /// It is small enough to hold whole, and is text.
    Yes,
    /// It is not offered for editing.
    #[default]
    No,
}

/// What the stage needs to know of the open file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextParams {
    /// The views the file has.
    pub views: TextViews,
    /// How long the file is and how much of it shows.
    pub extent: TextExtent,
    /// Whether the file can be edited.
    pub editable: Editable,
    /// Whether a file of this kind starts with its long lines wrapped: the person's last choice
    /// for the kind, else the kind's own.
    pub wrap: Wrap,
}
