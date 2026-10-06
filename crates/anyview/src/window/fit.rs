//! How large a window opens: its content's own size when it has one, within a cap and a least.
//! This is the one place a viewer window's size is decided; [`spec_for`](super::root::spec_for)
//! asks it once, as the window opens, and nothing resizes the window afterwards (moving to the
//! next file keeps the size, and a window a person resized is theirs).

use anyview_core::{FilePath, PixelSize};
use ds_blitz::{Extent, WindowSize};

/// The window of a file with no natural size (text, code, a table, a book, a folder, a failure): 1000
/// by 700 logical pixels.
pub(crate) const WINDOW: WindowSize = WindowSize::new(1000, 700);

/// The least a viewer window is resized to: 480 by 320. It is as wide as the mini window (480 by
/// 270) and a little taller, and narrower and shorter than the welcome window (520 by 380), so a
/// picture smaller than this stays centred on the background with room for the titlebar and capsule.
pub(crate) const LEAST: Extent = Extent::new(480, 320);

/// The default of [`fit_cap`]: what a window never grows past, 1600 by 1000 logical pixels.
const DEFAULT_CAP: Extent = Extent::new(1600, 1000);

/// The environment variable that replaces the cap, as `WIDTHxHEIGHT` (for testing).
const CAP_VARIABLE: &str = "ANYVIEW_WINDOW_CAP";

/// The largest content area a window opens at.
///
/// A fixed default until ds-blitz reports the screen: it becomes 85% of the work area of the
/// monitor the window opens on once quire exposes `screen_extent()`.
///
/// TODO(hidpi): a picture is natural at one image pixel to one physical pixel, as Preview shows it,
/// so the natural size divides by the monitor's scale factor before it is fitted. ds-blitz exposes
/// no monitor yet and the scale is not guessed: image pixels are taken as logical pixels.
pub(crate) fn fit_cap() -> Extent {
    std::env::var(CAP_VARIABLE)
        .ok()
        .and_then(|text| parse_cap(&text))
        .unwrap_or(DEFAULT_CAP)
}

/// `WIDTHxHEIGHT` as an extent; `None` when it is not two positive whole numbers.
fn parse_cap(text: &str) -> Option<Extent> {
    let (width, height) = text.trim().split_once(['x', 'X'])?;
    let (width, height) = (width.trim().parse().ok()?, height.trim().parse().ok()?);
    (width > 0 && height > 0).then(|| Extent::new(width, height))
}

/// The window a viewer opens on `file`: its content's natural size, fitted.
pub(crate) fn window_for(file: &FilePath) -> WindowSize {
    fitted(
        anyview_peek::natural_size(file).map(extent_of),
        fit_cap(),
        LEAST,
    )
}

fn extent_of(size: PixelSize) -> Extent {
    Extent::new(size.width.0, size.height.0)
}

/// The window for content of `natural` size: that size, scaled down to fit `cap` with its ratio
/// kept, and never below `least` on either axis (an axis the ratio would push below it is held at
/// it, and the content centres on the background). No natural size, or an empty one, is
/// [`WINDOW`]. Sizes are whole logical pixels.
pub(crate) fn fitted(natural: Option<Extent>, cap: Extent, least: Extent) -> WindowSize {
    let start = natural
        .filter(|natural| natural.width > 0 && natural.height > 0)
        .map_or(
            Extent::new(WINDOW.start().width, WINDOW.start().height),
            |n| scaled_into(n, cap),
        );
    WindowSize::new(start.width.max(least.width), start.height.max(least.height))
        .with_least(least.width, least.height)
}

/// `size` as it is when it fits `cap`, else scaled down to touch it on the side that limits it.
fn scaled_into(size: Extent, cap: Extent) -> Extent {
    if size.width <= cap.width && size.height <= cap.height {
        return size;
    }
    let (w, h) = (u128::from(size.width), u128::from(size.height));
    let (cw, ch) = (u128::from(cap.width), u128::from(cap.height));
    // Width limits when its ratio to the cap is the larger: w / cw >= h / ch.
    let (width, height) = if w * ch >= h * cw {
        (cw, (h * cw * 2 + w) / (w * 2))
    } else {
        ((w * ch * 2 + h) / (h * 2), ch)
    };
    let whole = |length: u128| u32::try_from(length).unwrap_or(u32::MAX);
    Extent::new(whole(width), whole(height))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAP: Extent = Extent::new(1600, 1000);

    fn start(size: WindowSize) -> (u32, u32) {
        (size.start().width, size.start().height)
    }

    fn of(width: u32, height: u32) -> (u32, u32) {
        start(fitted(Some(Extent::new(width, height)), CAP, LEAST))
    }

    #[test]
    fn no_natural_size_is_the_default_window_with_the_least() {
        assert_eq!(fitted(None, CAP, LEAST), WINDOW.with_least(480, 320));
        assert_eq!(
            fitted(Some(Extent::new(0, 40)), CAP, LEAST),
            WINDOW.with_least(480, 320),
            "an empty size is none"
        );
    }

    #[test]
    fn a_size_between_the_least_and_the_cap_is_taken_as_it_is() {
        assert_eq!(of(800, 600), (800, 600));
        assert_eq!(of(1600, 1000), (1600, 1000));
        assert_eq!(of(480, 320), (480, 320));
    }

    #[test]
    fn a_smaller_picture_gets_the_least_and_stays_centred() {
        assert_eq!(of(64, 64), (480, 320));
        assert_eq!(of(300, 900), (480, 900), "only the short axis is raised");
    }

    #[test]
    fn a_larger_picture_is_scaled_to_the_cap_keeping_its_ratio() {
        assert_eq!(of(3200, 1000), (1600, 500), "wider than the cap");
        assert_eq!(of(1000, 4000), (480, 1000), "taller than the cap, narrow");
        assert_eq!(of(4000, 3000), (1333, 1000), "the height limits");
        assert_eq!(of(6000, 3000), (1600, 800), "the width limits");
        assert_eq!(of(1700, 1000), (1600, 941), "rounded to whole pixels");
    }

    #[test]
    fn an_extreme_ratio_holds_the_short_axis_at_the_least() {
        assert_eq!(of(100_000, 10), (1600, 320));
        assert_eq!(of(10, 100_000), (480, 1000));
    }

    #[test]
    fn a_claim_of_a_hundred_thousand_pixels_each_way_just_meets_the_cap() {
        assert_eq!(of(100_000, 100_000), (1000, 1000));
        assert_eq!(of(u32::MAX, u32::MAX), (1000, 1000));
        assert_eq!(of(u32::MAX, 1), (1600, 320));
    }

    #[test]
    fn the_window_never_exceeds_the_cap_nor_falls_below_the_least() {
        for (w, h) in [
            (1, 1),
            (479, 319),
            (2000, 10),
            (10, 2000),
            (1601, 1001),
            (9999, 7),
        ] {
            let (width, height) = of(w, h);
            assert!((480..=1600).contains(&width), "{w}x{h} gave {width}");
            assert!((320..=1000).contains(&height), "{w}x{h} gave {height}");
        }
    }

    #[test]
    fn the_cap_is_set_as_width_x_height_and_anything_else_is_ignored() {
        assert_eq!(parse_cap("1280x800"), Some(Extent::new(1280, 800)));
        assert_eq!(parse_cap(" 640 X 480 "), Some(Extent::new(640, 480)));
        for bad in ["", "1280", "0x800", "1280x0", "-1x5", "axb", "1x2x3"] {
            assert_eq!(parse_cap(bad), None, "{bad:?}");
        }
    }
}
