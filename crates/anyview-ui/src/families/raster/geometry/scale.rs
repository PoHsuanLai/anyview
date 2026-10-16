//! How big a picture is shown and where it is centred: its turn, animation and frame as the stage
//! machine holds them, the scale that fits it to the room, and the unit conversions.

use crate::{Animation, Area, FrameIndex, RasterStage};
use anyview_core::{DocPoint, DocUnit, Permille, PixelSize, QuarterTurn, Zoom};

/// The picture's width and height in texels as shown: a quarter turn swaps them.
pub(crate) fn turned(size: PixelSize, turn: QuarterTurn) -> (f64, f64) {
    let (w, h) = (f64::from(size.width.0), f64::from(size.height.0));
    match turn {
        QuarterTurn::None | QuarterTurn::Half => (w, h),
        QuarterTurn::Quarter | QuarterTurn::ThreeQuarter => (h, w),
    }
}

/// The turn the stage is at.
pub(crate) fn turn_of(stage: &RasterStage) -> QuarterTurn {
    match stage {
        RasterStage::Fitted { turn, .. }
        | RasterStage::Zoomed { turn, .. }
        | RasterStage::Panning { turn, .. } => *turn,
    }
}

/// Whether the picture moves, and which frame it is on.
pub(crate) fn animation_of(stage: &RasterStage) -> Animation {
    match stage {
        RasterStage::Fitted { anim, .. }
        | RasterStage::Zoomed { anim, .. }
        | RasterStage::Panning { anim, .. } => *anim,
    }
}

/// The frame on screen: the one the animation is on, or the first.
pub(crate) fn frame_of(stage: &RasterStage) -> FrameIndex {
    match animation_of(stage) {
        Animation::Still => FrameIndex(0),
        Animation::Playing { frame, .. }
        | Animation::Paused { frame, .. }
        | Animation::Ended { frame, .. } => frame,
    }
}

/// Device pixels per texel at which the whole picture fits `area`, never above actual size: a
/// small picture is shown at its own size, not stretched.
pub(crate) fn fit(size: PixelSize, turn: QuarterTurn, area: Area) -> Permille {
    let (w, h) = turned(size, turn);
    let device = (
        f64::from(area.size.width.0) * f64::from(area.scale),
        f64::from(area.size.height.0) * f64::from(area.scale),
    );
    let scale = (device.0 / w.max(1.0)).min(device.1 / h.max(1.0)).min(1.0);
    Permille(per_mille(scale).max(Zoom::MIN_SCALE.0))
}

/// The scale the stage draws at, in device pixels per texel.
pub(crate) fn scale_of(stage: &RasterStage, fit: Permille) -> Permille {
    match stage {
        RasterStage::Fitted { .. } => fit,
        RasterStage::Zoomed { zoom, .. } | RasterStage::Panning { zoom, .. } => match zoom {
            Zoom::Fit | Zoom::Fill => fit,
            Zoom::Actual => Permille::WHOLE,
            Zoom::Scale(scale) => *scale,
        },
    }
}

/// The picture point in the middle of the room, as shown: for a picture of `size` turned by
/// `turn`, the middle of it when it is fitted.
pub(crate) fn centre_of(stage: &RasterStage, size: PixelSize, turn: QuarterTurn) -> DocPoint {
    match stage {
        RasterStage::Zoomed { centre, .. } | RasterStage::Panning { centre, .. } => *centre,
        RasterStage::Fitted { .. } => {
            let (w, h) = turned(size, turn);
            DocPoint {
                x: units(w / 2.0),
                y: units(h / 2.0),
            }
        }
    }
}

fn per_mille(scale: f64) -> u32 {
    // A scale is positive and below 2^32 / 1000 whatever the picture, so the cast cannot wrap.
    (scale * 1000.0).round().clamp(0.0, f64::from(u32::MAX)) as u32
}

pub(super) fn units(texels: f64) -> DocUnit {
    DocUnit((texels * f64::from(DocUnit::PER_PIXEL)).round() as i32)
}

pub(super) fn texels(point: DocUnit) -> f64 {
    f64::from(point.0) / f64::from(DocUnit::PER_PIXEL)
}
