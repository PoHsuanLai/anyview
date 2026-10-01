//! What the palette can run: a file action, or a command for the stage that is showing.

use anyview_core::FileAction;
use ds_core::word::Word;

/// A command a stage understands. The stage that is showing decides what it means, or that it
/// means nothing (a PDF has no "toggle source").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum StageCommand {
    /// Zoom in one step.
    ZoomIn,
    /// Zoom out one step.
    ZoomOut,
    /// Fit the content to the window.
    ZoomToFit,
    /// Show the content at its own size.
    ZoomToActual,
    /// Start a find.
    Find,
    /// Go to the next find hit.
    FindNext,
    /// Go to the previous find hit.
    FindPrevious,
    /// Switch a text document between rendered and source.
    ToggleSource,
    /// Switch line wrapping.
    ToggleWrap,
    /// Play or pause.
    TogglePlayback,
}

/// One row the palette can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Command {
    /// An action every file has some of; the one list in `anyview-core`.
    File(FileAction),
    /// A command for the stage.
    Stage(StageCommand),
}
