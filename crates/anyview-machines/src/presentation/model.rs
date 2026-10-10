//! Presentation's states, inputs and outputs.

/// How the viewer is on screen. The one a viewer starts in comes from how it was launched; the
/// transitions below are the only ones the design allows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Presentation {
    /// A normal window.
    #[default]
    Window,
    /// The launcher's quick look: a window that is promoted to `Window` and nothing else.
    Peek,
    /// A small borderless video window with capsule controls; media only.
    Mini,
    /// Playing with no window, publishing the media session.
    Background,
    /// A region of a host's window: no frame of its own, no sheets, no palette and none of the
    /// viewer's chords, since the host owns them. It stays a pane; the host decides whether a
    /// viewer is shown this way.
    Pane,
}

/// What moves the presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationIn {
    /// Show a full window: promote a peek, expand the mini window, or open a background
    /// session.
    ToWindow,
    /// Shrink the window to the mini window.
    ToMini,
    /// The clock; presentation keeps no timer.
    Elapsed,
}

impl From<ds_core::machine::Elapsed> for PresentationIn {
    fn from(_: ds_core::machine::Elapsed) -> Self {
        PresentationIn::Elapsed
    }
}

/// What presentation wants done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationOut {
    /// Put the viewer on screen this way: create, resize or restyle its window.
    Become(Presentation),
}

/// What is open, as far as presentation cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContentClass {
    /// Video or audio: it can have a mini window.
    Media,
    /// Anything else.
    #[default]
    Document,
}

/// What presentation needs to know of the open file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PresentationParams {
    /// Whether the file is media.
    pub content: ContentClass,
}
