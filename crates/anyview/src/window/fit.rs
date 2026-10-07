//! How large a window opens: its content's own size when it has one, within a cap and a least.
//! This is the one place a viewer window's size is decided. [`window_for`] asks it as the window
//! opens, for the size the file's header gives; [`WindowFit`] asks it once more, after the first
//! file has loaded, for content whose size only the loaded document knows (a PDF's page, a picture
//! a plugin decoded). Nothing else resizes the window: moving to the next file keeps the size,
//! and a window a person resized is theirs.

use anyview_core::{FilePath, PixelSize};
use ds_blitz::{Extent, SizeOrigin, WindowSize, WindowSizer};

/// The window of a file with no natural size (text, code, a table, a book, a folder, a failure): 1000
/// by 700 logical pixels.
pub(crate) const WINDOW: WindowSize = WindowSize::new(1000, 700);

/// The least a viewer window is resized to: 480 by 320. It is as wide as the mini window (480 by
/// 270) and a little taller, and narrower and shorter than the welcome window (520 by 380), so a
/// picture smaller than this stays centred on the background with room for the titlebar and capsule.
pub(crate) const LEAST: Extent = Extent::new(480, 320);

/// The cap when the screen is not known (the first window opens before the event loop runs): 1600
/// by 1000 logical pixels.
const DEFAULT_CAP: Extent = Extent::new(1600, 1000);

/// The environment variable that replaces the cap, as `WIDTHxHEIGHT` (for testing).
const CAP_VARIABLE: &str = "ANYVIEW_WINDOW_CAP";

/// The largest content area a window opens at, for a monitor of `screen` logical pixels: quire's
/// share of it (`Extent::fit`, 85%). With no screen (the event loop is not up, or winit lists no
/// monitor) it is the fixed [`DEFAULT_CAP`]. `ANYVIEW_WINDOW_CAP` replaces either.
///
/// `screen` is `AppHandle::screen_extent()`: logical pixels, so the cap is right at any integer
/// scale (on Wayland a fractional output is off by the difference, quire says).
///
/// TODO(hidpi): a picture is natural at one image pixel to one physical pixel, as Preview shows it,
/// so the natural size would divide by the monitor's scale factor before it is fitted. quire
/// exposes the screen's logical size and no scale factor, so image pixels are taken as logical
/// pixels until it does.
pub(crate) fn cap_for(screen: Option<Extent>) -> Extent {
    let replaced = std::env::var(CAP_VARIABLE)
        .ok()
        .and_then(|text| parse_cap(&text));
    cap_from(screen, replaced)
}

/// [`cap_for`] given what the environment says.
fn cap_from(screen: Option<Extent>, replaced: Option<Extent>) -> Extent {
    replaced
        .or_else(|| screen.map(cap_of))
        .unwrap_or(DEFAULT_CAP)
}

/// What quire caps a window to on a screen of `screen`: its `Extent::fit` of an unbounded size.
fn cap_of(screen: Extent) -> Extent {
    Extent::new(u32::MAX, u32::MAX).fit(screen, None)
}

/// `WIDTHxHEIGHT` as an extent; `None` when it is not two positive whole numbers.
fn parse_cap(text: &str) -> Option<Extent> {
    let (width, height) = text.trim().split_once(['x', 'X'])?;
    let (width, height) = (width.trim().parse().ok()?, height.trim().parse().ok()?);
    (width > 0 && height > 0).then(|| Extent::new(width, height))
}

/// The window a viewer opens on `file` on a `screen` (see [`cap_for`]): its content's natural size
/// by its header, fitted.
pub(crate) fn window_for(file: &FilePath, screen: Option<Extent>) -> WindowSize {
    fitted(
        anyview_peek::natural_size(file).map(extent_of),
        cap_for(screen),
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
            Extent::new(
                WINDOW.start().width.min(cap.width),
                WINDOW.start().height.min(cap.height),
            ),
            |n| scaled_into(n, cap),
        );
    WindowSize::fitting(start, None, least)
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

/// How the window's size is asked for and read after it opened: quire's [`WindowSizer`], or a
/// test's stand-in (a harness window has no sizer).
pub trait Sizer {
    /// Who resized the window last: `None` while nobody has, `Person` for any size the program
    /// did not ask for.
    fn origin(&self) -> Option<SizeOrigin>;
    /// The window's size now, in logical pixels.
    fn size(&self) -> Extent;
    /// Ask the window to be `size` logical pixels.
    fn request(&self, size: Extent);
}

impl Sizer for WindowSizer {
    fn origin(&self) -> Option<SizeOrigin> {
        WindowSizer::origin(self)
    }

    fn size(&self) -> Extent {
        WindowSizer::size(self)
    }

    fn request(&self, size: Extent) {
        self.request_size(size);
    }
}

/// A [`Sizer`] a test provides as a root context in place of quire's: a harness window has no
/// window to size. It is made inside the window, since a sizer is not `Send`.
#[derive(Clone)]
pub struct SizerContext(std::sync::Arc<dyn Fn() -> std::rc::Rc<dyn Sizer> + Send + Sync>);

impl std::fmt::Debug for SizerContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SizerContext").finish_non_exhaustive()
    }
}

impl SizerContext {
    /// A context that gives the window the sizer `make` makes.
    pub fn new(make: impl Fn() -> std::rc::Rc<dyn Sizer> + Send + Sync + 'static) -> SizerContext {
        SizerContext(std::sync::Arc::new(make))
    }

    pub(crate) fn make(&self) -> std::rc::Rc<dyn Sizer> {
        (self.0)()
    }
}

/// The one resize after load: a window that opened at a default size, because its file's header
/// did not give one, takes its content's natural size once the first file has loaded. It asks at
/// most once, never once the person has resized the window, and never for a later file (moving
/// with the arrows keeps the window, as Preview does).
pub(crate) struct WindowFit {
    sizer: std::rc::Rc<dyn Sizer>,
    asked: std::cell::Cell<bool>,
}

impl WindowFit {
    pub(crate) fn new(sizer: std::rc::Rc<dyn Sizer>) -> WindowFit {
        WindowFit {
            sizer,
            asked: std::cell::Cell::new(false),
        }
    }

    /// The window's first file has loaded and its content is `natural` big: size the window to it
    /// on a `screen` (see [`cap_for`]). Returns whether it asked for a size.
    pub(crate) fn loaded(&self, natural: PixelSize, screen: Option<Extent>) -> bool {
        if self.asked.replace(true) || self.sizer.origin() == Some(SizeOrigin::Person) {
            return false;
        }
        let wanted = fitted(Some(extent_of(natural)), cap_for(screen), LEAST).start();
        // The window already is that size (the header gave it): nothing to ask.
        if wanted == self.sizer.size() {
            return false;
        }
        self.sizer.request(wanted);
        true
    }
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
    fn the_cap_is_85_percent_of_the_screen_through_quire_and_the_fixed_one_without() {
        const FAKE: Extent = Extent::new(2000, 1000);
        assert_eq!(cap_from(Some(FAKE), None), Extent::new(1700, 850));
        assert_eq!(
            cap_from(Some(Extent::new(1280, 720)), None),
            Extent::new(1088, 612)
        );
        assert_eq!(
            cap_from(None, None),
            DEFAULT_CAP,
            "before the event loop runs"
        );
        assert_eq!(
            cap_from(Some(FAKE), Some(Extent::new(640, 480))),
            Extent::new(640, 480),
            "the override wins"
        );
    }

    #[test]
    fn a_window_is_fitted_through_quires_fitting_on_a_fake_screen() {
        const SCREEN: Extent = Extent::new(2000, 1000);
        let on_screen =
            |natural: Option<Extent>| fitted(natural, cap_from(Some(SCREEN), None), LEAST).start();
        assert_eq!(
            on_screen(Some(Extent::new(800, 600))),
            Extent::new(800, 600)
        );
        assert_eq!(
            on_screen(Some(Extent::new(4000, 3000))),
            Extent::new(1133, 850),
            "scaled to the 85% cap, ratio kept"
        );
        assert_eq!(on_screen(Some(Extent::new(64, 64))), LEAST);
        assert_eq!(
            on_screen(None),
            Extent::new(1000, 700),
            "the default window fits"
        );
        // A small screen holds even the default window to its 85%.
        let small = fitted(None, cap_from(Some(Extent::new(1000, 600)), None), LEAST);
        assert_eq!(small.start(), Extent::new(850, 510));
        // The window's own least is quire's.
        assert_eq!(small.least(), Some(LEAST));
    }

    /// A sizer that records the sizes it is asked for.
    struct Recording {
        origin: std::cell::Cell<Option<SizeOrigin>>,
        size: Extent,
        asked: std::cell::RefCell<Vec<Extent>>,
    }

    impl Sizer for Recording {
        fn origin(&self) -> Option<SizeOrigin> {
            self.origin.get()
        }

        fn size(&self) -> Extent {
            self.size
        }

        fn request(&self, size: Extent) {
            self.asked.borrow_mut().push(size);
        }
    }

    fn fit_with(origin: Option<SizeOrigin>, size: Extent) -> (std::rc::Rc<Recording>, WindowFit) {
        let sizer = std::rc::Rc::new(Recording {
            origin: std::cell::Cell::new(origin),
            size,
            asked: std::cell::RefCell::new(Vec::new()),
        });
        let fit = WindowFit::new(std::rc::Rc::clone(&sizer) as std::rc::Rc<dyn Sizer>);
        (sizer, fit)
    }

    const PAGE: PixelSize = PixelSize {
        width: anyview_core::PixelLen(612),
        height: anyview_core::PixelLen(792),
    };

    #[test]
    fn the_first_load_asks_once_and_only_once() {
        let (sizer, fit) = fit_with(None, Extent::new(1000, 700));
        assert!(fit.loaded(PAGE, Some(Extent::new(2000, 1000))));
        assert!(
            !fit.loaded(PAGE, Some(Extent::new(2000, 1000))),
            "a second load"
        );
        // The page fits the 1700 by 850 cap, so it is its own size.
        assert_eq!(*sizer.asked.borrow(), vec![Extent::new(612, 792)]);
    }

    #[test]
    fn a_page_taller_than_the_screen_is_scaled_to_its_cap() {
        let (sizer, fit) = fit_with(None, Extent::new(1000, 700));
        assert!(fit.loaded(PAGE, Some(Extent::new(1600, 900))));
        // The cap is 1360 by 765: 612 by 792 scales to 591 by 765.
        assert_eq!(*sizer.asked.borrow(), vec![Extent::new(591, 765)]);
    }

    #[test]
    fn a_window_the_person_resized_is_not_asked_to_resize() {
        let (sizer, fit) = fit_with(Some(SizeOrigin::Person), Extent::new(1000, 700));
        assert!(!fit.loaded(PAGE, None));
        assert!(sizer.asked.borrow().is_empty());
    }

    #[test]
    fn a_window_already_that_size_is_not_asked() {
        let (sizer, fit) = fit_with(Some(SizeOrigin::Requested), Extent::new(612, 792));
        assert!(!fit.loaded(PAGE, None));
        assert!(sizer.asked.borrow().is_empty());
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
