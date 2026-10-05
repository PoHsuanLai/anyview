//! The raster stage's states, inputs and outputs.
//!
//! The plan lists `Animating` beside `Fitted`, `Zoomed` and `Panning`. Playing is independent of
//! framing (a GIF can be zoomed and dragged while it plays), so as a fourth state it would need a
//! copy of every framing state; here it is an [`Animation`] each framing state carries.

use super::super::zoom::{Viewport, ZoomDir};
use anyview_core::{DocPoint, Permille, QuarterTurn, Resume, Zoom};
use std::num::NonZeroU32;

/// A frame of an animated image, from 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct FrameIndex(pub u32);

/// How many frames an animated image has; at least one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameCount(pub NonZeroU32);

/// Whether the image moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Animation {
    /// One picture.
    Still,
    /// Advancing: the edge sends a tick when the current frame's time is up.
    Playing { frame: FrameIndex, of: FrameCount },
    /// Held on a frame.
    Paused { frame: FrameIndex, of: FrameCount },
}

/// How the image is framed. `turn` is how far it has been rotated this session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterStage {
    /// Fitted to the window, centred.
    Fitted { turn: QuarterTurn, anim: Animation },
    /// Drawn at `zoom` (never `Zoom::Fit`) with `centre` the content point at the middle of the
    /// view.
    Zoomed {
        turn: QuarterTurn,
        zoom: Zoom,
        centre: DocPoint,
        anim: Animation,
    },
    /// A drag is moving `centre` of a zoomed image.
    Panning {
        turn: QuarterTurn,
        zoom: Zoom,
        centre: DocPoint,
        anim: Animation,
    },
}

impl Default for RasterStage {
    fn default() -> Self {
        RasterStage::Fitted {
            turn: QuarterTurn::None,
            anim: Animation::Still,
        }
    }
}

/// What moves the stage. Points are in the content's own space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterIn {
    /// One zoom step in or out, holding `at` still on screen.
    ZoomStep { dir: ZoomDir, at: DocPoint },
    /// Zoom to `zoom`, holding `at` still on screen.
    SetZoom { zoom: Zoom, at: DocPoint },
    /// Fitted goes to 1:1 at `at`; any zoom goes back to fit.
    DoubleClick { at: DocPoint },
    /// A drag began.
    PanStart,
    /// The drag moved the pointer by this much; the content follows it.
    PanBy(DocPoint),
    /// The drag ended.
    PanEnd,
    /// Restore where the person left the file.
    Restore { zoom: Zoom, centre: DocPoint },
    /// The file turned out to be animated.
    Animated(FrameCount),
    /// The current frame's time is up.
    FrameTick,
    /// Play or pause an animation.
    TogglePlayback,
    /// The clock; the stage keeps no timer.
    Elapsed,
}

impl From<ds_core::machine::Elapsed> for RasterIn {
    fn from(_: ds_core::machine::Elapsed) -> Self {
        RasterIn::Elapsed
    }
}

/// What the stage wants done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RasterOut {
    /// Keep where the person is, for next time. Sent when a gesture settles, not while a drag
    /// moves.
    Remember(Resume),
    /// Draw this frame.
    ShowFrame(FrameIndex),
}

/// What the stage needs from the view and from settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RasterParams {
    /// The scale on screen and the fit scale.
    pub viewport: Viewport,
    /// The content point at the middle of the view now.
    pub centre: DocPoint,
    /// A zoom step's factor in thousandths (setting `viewer.zoom.step`, default 1250).
    pub step: Permille,
}

impl Default for RasterParams {
    fn default() -> Self {
        RasterParams {
            viewport: Viewport {
                shown: Permille::WHOLE,
                fit: Permille::WHOLE,
            },
            centre: DocPoint::default(),
            step: Permille(1250),
        }
    }
}
