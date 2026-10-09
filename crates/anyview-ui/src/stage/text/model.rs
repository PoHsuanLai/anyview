//! The text stage's states, inputs and outputs.

use super::super::find::{FindHits, FindOut, HitCount, HitIndex};
use crate::typed::TypedText;
use anyview_core::{LineIndex, Resume};
use ds_core::word::Word;

/// Whether long lines wrap at the window's edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum Wrap {
    /// Lines wrap.
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
}

impl TextStage {
    /// Where the reader is and how the text is shown.
    pub fn place(&self) -> TextPlace {
        match self {
            TextStage::Reading { place } | TextStage::Finding { place, .. } => *place,
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

/// What the stage needs to know of the open file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextParams {
    /// The views the file has.
    pub views: TextViews,
    /// How long the file is and how much of it shows.
    pub extent: TextExtent,
}
