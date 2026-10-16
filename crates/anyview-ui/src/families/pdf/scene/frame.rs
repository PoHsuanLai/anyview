//! The room the pages are laid out in, and the scales that fit them to it.

use crate::Area;
use anyview_core::{Permille, Zoom};
use anyview_pdf::PageSize;

/// The gap between two pages, in logical pixels.
pub(in crate::families::pdf) const GAP: u32 = 12;
/// The room left around a page that is fitted, in logical pixels.
pub(in crate::families::pdf) const MARGIN: u32 = 16;
/// The room in device pixels, and how many device pixels a logical one is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::families::pdf) struct Frame {
    pub width: u32,
    pub height: u32,
    /// Device pixels per logical pixel, in thousandths.
    pub density: Permille,
}
impl Frame {
    /// The room `area` measured.
    pub(in crate::families::pdf) fn of(area: Area) -> Frame {
        let density = per_mille(f64::from(area.scale)).max(1);
        let device = |logical: f32| per_mille(f64::from(logical) * f64::from(area.scale)) / 1000;
        Frame {
            width: device(area.size.width.0).max(1),
            height: device(area.size.height.0).max(1),
            density: Permille(density),
        }
    }

    /// `logical` pixels as device pixels.
    pub(in crate::families::pdf) fn device(self, logical: u32) -> u32 {
        u32::try_from(u64::from(logical) * u64::from(self.density.0) / 1000).unwrap_or(u32::MAX)
    }

    /// `device` pixels as the logical pixels CSS counts.
    pub(in crate::families::pdf) fn css(self, device: i64) -> f32 {
        device as f32 * 1000.0 / self.density.0.max(1) as f32
    }
}
fn per_mille(value: f64) -> u32 {
    // A scale or a length is positive and far below 2^32 / 1000, so the cast cannot wrap.
    (value * 1000.0).round().clamp(0.0, f64::from(u32::MAX)) as u32
}
/// The two scales a zoom can mean for this document in this room.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::families::pdf) struct Fits {
    /// The widest page fills the room's width.
    pub width: Permille,
    /// The largest page fits the room whole.
    pub page: Permille,
}
/// The scales at which `sizes` fit `frame`: the margin is kept on every side.
pub(in crate::families::pdf) fn fits(sizes: &[PageSize], frame: Frame) -> Fits {
    let wide = sizes
        .iter()
        .map(|size| size.width.0)
        .max()
        .unwrap_or(1)
        .max(1);
    let tall = sizes
        .iter()
        .map(|size| size.height.0)
        .max()
        .unwrap_or(1)
        .max(1);
    let margin = 2 * frame.device(MARGIN);
    let across = |room: u32, points: u32| {
        let room = u64::from(room.saturating_sub(margin).max(1));
        let scale = room * 1_000_000 / u64::from(points);
        Permille(u32::try_from(scale).unwrap_or(u32::MAX))
    };
    let width = across(frame.width, wide);
    let page = Permille(width.0.min(across(frame.height, tall).0));
    Fits {
        width: within_limits(width),
        page: within_limits(page),
    }
}
/// `scale` kept inside the zoom limits.
fn within_limits(scale: Permille) -> Permille {
    Permille(scale.0.clamp(Zoom::MIN_SCALE.0, Zoom::MAX_SCALE.0))
}
/// The scale `zoom` draws at.
pub(in crate::families::pdf) fn scale_of(zoom: Zoom, fits: Fits) -> Permille {
    match zoom {
        Zoom::Fit => fits.page,
        Zoom::Fill => fits.width,
        Zoom::Actual => Permille::WHOLE,
        Zoom::Scale(scale) => within_limits(scale),
    }
}
/// The scale after a pinch or a wheel turn of `by` thousandths, kept inside the zoom limits.
pub(in crate::families::pdf) fn pinched(shown: Permille, by: i32) -> Permille {
    let grown = i64::from(shown.0) * (1000 + i64::from(by.clamp(-900, 4000))) / 1000;
    within_limits(Permille(u32::try_from(grown).unwrap_or(Zoom::MIN_SCALE.0)))
}
