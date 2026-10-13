//! The raster stage's states, inputs and outputs.
//!
//! The plan lists `Animating` beside `Fitted`, `Zoomed` and `Panning`. Playing is independent of
//! framing (a GIF can be zoomed and dragged while it plays), so as a fourth state it would need a
//! copy of every framing state; here it is an [`Animation`] each framing state carries.

use super::super::media::StepDirection;
use super::super::zoom::{Viewport, ZoomDir};
use anyview_core::{DocPoint, Permille, QuarterTurn, Resume, Zoom};
use ds_core::time::stamp::Stamp;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

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
    /// Advancing: `frame` is on screen until `due`, and `run` runs have finished before this one.
    Playing {
        /// The frame on screen.
        frame: FrameIndex,
        /// How many frames there are.
        of: FrameCount,
        /// When the frame gives way to the next.
        due: Stamp,
        /// How many runs have finished.
        run: u32,
    },
    /// Held on a frame, in the run after `run` finished ones.
    Paused {
        /// The frame on screen.
        frame: FrameIndex,
        /// How many frames there are.
        of: FrameCount,
        /// How many runs have finished.
        run: u32,
    },
    /// Every run the file asked for is done, and the last frame stays.
    Ended {
        /// The frame on screen.
        frame: FrameIndex,
        /// How many frames there are.
        of: FrameCount,
    },
}

/// How many times an animation runs through its frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Runs {
    /// Until the person stops it.
    #[default]
    Forever,
    /// This many, then it holds on the last frame.
    Times(NonZeroU32),
}

/// Whether the desktop asks for less movement. An animation then waits for the person to play it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Motion {
    /// Animations play as they open.
    #[default]
    Standard,
    /// Animations open paused on their first frame.
    Reduced,
}

/// How long each frame of the open animation stays, in order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FrameDelays(pub Arc<[Duration]>);

/// How the image is framed. `turn` is how far it has been rotated this session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterStage {
    /// Fitted to the window, centred.
    Fitted {
        /// How far the image has been turned.
        turn: QuarterTurn,
        /// Whether the image moves.
        anim: Animation,
    },
    /// Drawn at `zoom` (never `Zoom::Fit`) with `centre` the content point at the middle of the
    /// view.
    Zoomed {
        /// How far the image has been turned.
        turn: QuarterTurn,
        /// How far in the image is drawn.
        zoom: Zoom,
        /// The content point at the middle of the view.
        centre: DocPoint,
        /// Whether the image moves.
        anim: Animation,
    },
    /// A drag is moving `centre` of a zoomed image.
    Panning {
        /// How far the image has been turned.
        turn: QuarterTurn,
        /// How far in the image is drawn.
        zoom: Zoom,
        /// The content point at the middle of the view.
        centre: DocPoint,
        /// Whether the image moves.
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

impl RasterStage {
    /// Whether the open image moves: only then are playback and frame stepping its commands.
    #[must_use]
    pub fn is_animated(&self) -> bool {
        let (RasterStage::Fitted { anim, .. }
        | RasterStage::Zoomed { anim, .. }
        | RasterStage::Panning { anim, .. }) = self;
        !matches!(anim, Animation::Still)
    }
}

/// What moves the stage. Points are in the content's own space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterIn {
    /// One zoom step in or out, holding `at` still on screen.
    ZoomStep {
        /// In or out.
        dir: ZoomDir,
        /// The point held still on screen.
        at: DocPoint,
    },
    /// Zoom to `zoom`, holding `at` still on screen.
    SetZoom {
        /// The zoom to set.
        zoom: Zoom,
        /// The point held still on screen.
        at: DocPoint,
    },
    /// Fitted goes to 1:1 at `at`; any zoom goes back to fit.
    DoubleClick {
        /// The point held still on screen.
        at: DocPoint,
    },
    /// A drag began.
    PanStart,
    /// The drag moved the pointer by this much; the content follows it.
    PanBy(DocPoint),
    /// The drag ended.
    PanEnd,
    /// Restore where the person left the file.
    Restore {
        /// The zoom to go back to.
        zoom: Zoom,
        /// The content point at the middle of the view.
        centre: DocPoint,
    },
    /// The file turned out to be animated.
    Animated(FrameCount),
    /// Play or pause an animation; one that has ended starts again.
    TogglePlayback,
    /// Pause on the frame before or after the one shown, wrapping round.
    StepFrame(StepDirection),
    /// The clock: the shown frame's time may be up.
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterParams {
    /// The scale on screen and the fit scale.
    pub viewport: Viewport,
    /// The content point at the middle of the view now.
    pub centre: DocPoint,
    /// A zoom step's factor in thousandths (setting `viewer.zoom.step`, default 1250).
    pub step: Permille,
    /// How long each frame of an animation stays.
    pub delays: FrameDelays,
    /// How many runs the animation asks for.
    pub runs: Runs,
    /// Whether the desktop asks for less movement.
    pub motion: Motion,
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
            delays: FrameDelays::default(),
            runs: Runs::default(),
            motion: Motion::default(),
        }
    }
}
