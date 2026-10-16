//! The picture's box in the room, how it is sampled and turned, and where a pointer is on it.

use super::super::geometry::{centre_of, fit, point_under, scale_of, turn_of};
use crate::RasterStage;
use crate::families::view::{Area, StageCx};
use anyview_core::{DocPoint, Permille, PixelSize, QuarterTurn};
use ds::prelude::Point;
use ds_blitz::Sampling;

/// Whether `at`, a point of the window, is over the room.
pub(super) fn over(area: Area, at: Point) -> bool {
    let (x, y) = (at.x.0 - area.origin.x.0, at.y.0 - area.origin.y.0);
    x >= 0.0 && y >= 0.0 && x < area.size.width.0 && y < area.size.height.0
}
/// How the texture is resampled at `scale`: smooth when shrunk, hard pixels when blown up.
pub(super) fn sampling_at(scale: Permille) -> Sampling {
    match scale.0 {
        0..1000 => Sampling::Bicubic,
        1000..3000 => Sampling::Bilinear,
        _ => Sampling::Nearest,
    }
}
/// The CSS rotation of the texture's box.
pub(super) fn rotation(turn: QuarterTurn) -> u16 {
    turn.degrees()
}
/// How near an edge of the crop rectangle still takes hold of it, in logical pixels of the room.
pub(super) const CROP_REACH: f32 = 10.0;
/// The picture as the stage lays it out: the part of the file's picture that is kept, and how far
/// it is turned (by the stage and by the person's edits).
pub(super) fn laid_out(
    cx: &StageCx,
    stage: &RasterStage,
    size: PixelSize,
) -> (PixelSize, QuarterTurn) {
    let adjust = cx.picture.adjust();
    (adjust.kept(size).size, turn_of(stage).then(adjust.turn))
}
/// The point of the picture as shown that is under (`at`, in the room) now.
pub(super) fn under(
    cx: &StageCx,
    stage: &RasterStage,
    size: PixelSize,
    area: Area,
    at: (f32, f32),
) -> DocPoint {
    let (kept, turn) = laid_out(cx, stage, size);
    let shown = scale_of(stage, fit(kept, turn, area));
    point_under(centre_of(stage, kept, turn), shown, area, at)
}
