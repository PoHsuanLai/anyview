//! How large a window opens: its content's own size when it has one, within the screen and a
//! least. This is the one place a viewer window's size is decided. [`window_for`] asks it as the
//! window opens, for the size the file's header gives; [`WindowFit`] asks it again, after each
//! file has loaded, for content whose size only the loaded document knows (a PDF's page, a
//! picture a plugin decoded), and again for every different file the window then shows. Nothing
//! else resizes the window.

use anyview_core::{FilePath, PixelSize};
use anyview_fs::OnDisk;
use anyview_peek::{is_audio, natural_size};
use anyview_ui::{NaturalSize, SizeBasis, audio_window_size};
use ds::window::tiled::Tiled;
use ds::window::vocab::{Fullscreen, Maximized, WindowState};
use ds_blitz::{Extent, Fit, Reserve, ScreenArea, WindowSize, WindowSizer};

/// The window of a file with no natural size (text, code, a table, a book, a folder, a failure): 1000
/// by 700 logical pixels.
pub(crate) const WINDOW: WindowSize = WindowSize::new(1000, 700);

/// The least a viewer window is resized to: 480 by 320. It is as wide as the mini window (480 by
/// 270) and a little taller, and narrower and shorter than the welcome window (520 by 380), so a
/// picture smaller than this stays centred on the background with room for the titlebar and capsule.
pub(crate) const LEAST: Extent = Extent::new(480, 320);

/// The least the window of an audio file is resized to: 320 by 240. A player's window holds a
/// cover or a symbol and a capsule, so it may be smaller than a picture's.
pub(crate) const COMPACT_LEAST: Extent = Extent::new(320, 240);

/// The screen when none is known (the first window opens before the event loop runs): 1920 by
/// 1200 logical pixels, so the 85% cap is 1632 by 1020.
const DEFAULT_SCREEN: Extent = Extent::new(1920, 1200);

/// What the desktop's top bar takes off the screen, as far as a client can know: 32 logical pixels,
/// the height of GNOME's bar (KDE's panel and macOS's menu bar are about as tall). Wayland reports
/// no panel or dock to a client, so without this the cap would be 85% of the whole output.
const TOP_BAR: Reserve = Reserve {
    top: 32,
    right: 0,
    bottom: 0,
    left: 0,
};

/// The area a window is fitted to on `screen`: its work area less the top bar, in logical pixels,
/// or [`DEFAULT_SCREEN`] when no screen is known. `replaced` (the screen a test names) replaces
/// either. quire caps a window to 85% of it (`Extent::fit_with`).
pub(crate) fn work_for(screen: Option<ScreenArea>, replaced: Option<Extent>) -> Extent {
    replaced
        .or_else(|| screen.map(|area| area.less(TOP_BAR).work))
        .unwrap_or(DEFAULT_SCREEN)
}

/// `WIDTHxHEIGHT` as an extent; `None` when it is not two positive whole numbers.
pub(crate) fn parse_screen(text: &str) -> Option<Extent> {
    let (width, height) = text.trim().split_once(['x', 'X'])?;
    let (width, height) = (width.trim().parse().ok()?, height.trim().parse().ok()?);
    (width > 0 && height > 0).then(|| Extent::new(width, height))
}

/// The window a viewer opens on `file` on a `screen`: its content's natural size by its header,
/// fitted.
pub(crate) fn window_for(
    file: &FilePath,
    screen: Option<ScreenArea>,
    replaced: Option<Extent>,
) -> WindowSize {
    let natural = natural_of(file);
    fitted(
        natural.map(|natural| logical_of(natural, screen)),
        work_for(screen, replaced),
        natural.map_or(LEAST, least_of),
    )
}

/// What the file's header gives its window: a picture's or a movie's size, or an audio file's
/// compact window.
fn natural_of(file: &FilePath) -> Option<NaturalSize> {
    if let Some(size) = natural_size(file.on_disk()) {
        return Some(NaturalSize::Pixels(size));
    }
    is_audio(file.on_disk()).then(|| NaturalSize::Compact(audio_window_size()))
}

/// The least a window of `natural` content is resized to.
fn least_of(natural: NaturalSize) -> Extent {
    match natural {
        NaturalSize::Pixels(_) | NaturalSize::Points(_) => LEAST,
        NaturalSize::Compact(_) => COMPACT_LEAST,
    }
}

fn extent_of(size: PixelSize) -> Extent {
    Extent::new(size.width.0, size.height.0)
}

/// `natural` in logical pixels on `screen`: pictures show one image pixel to one device pixel, as
/// Preview shows them (a 1200 by 800 picture is a 600 by 400 window on a 2x output). With no
/// screen known the scale is taken as 1.
fn logical_of(natural: NaturalSize, screen: Option<ScreenArea>) -> Extent {
    match (natural, screen) {
        (NaturalSize::Pixels(size), Some(area)) => area.logical_for_pixels(extent_of(size)),
        (NaturalSize::Pixels(size) | NaturalSize::Points(size) | NaturalSize::Compact(size), _) => {
            extent_of(size)
        }
    }
}

/// The window for content of `natural` logical size on a `work` area: that size, scaled down to
/// quire's share of the area with its ratio kept, and never below `least` on either axis (an axis
/// the ratio would push below it is held at it, and the content centres on the background). No
/// natural size, or an empty one, is [`WINDOW`], held to the share per axis. Sizes are whole
/// logical pixels.
pub(crate) fn fitted(natural: Option<Extent>, work: Extent, least: Extent) -> WindowSize {
    match natural.filter(|natural| natural.width > 0 && natural.height > 0) {
        Some(natural) => WindowSize::fitting_with(natural, Some(work), least, Fit::KeepRatio),
        None => WindowSize::fitting_with(WINDOW.start(), Some(work), least, Fit::PerAxis),
    }
}

/// The resize after each load: the window takes the natural size of the file that has just landed,
/// as it did when the first opened, so a different file never inherits the last one's window. A
/// size the person set by hand does not hold: Preview sizes its window to each document, and the
/// owner asked for the same. The same file loaded again does not ask (the viewer sends nothing).
pub(crate) struct WindowFit {
    sizer: WindowSizer,
    /// The screen a test names in place of the desktop's.
    replaced: Option<Extent>,
}

impl WindowFit {
    pub(crate) fn new(sizer: WindowSizer, replaced: Option<Extent>) -> WindowFit {
        WindowFit { sizer, replaced }
    }

    /// A different file has loaded: size the window to its content on the screen it is on (exact
    /// once it is mapped, which it is by now). The size is `natural`, and the default window when
    /// there is none (a text, a table, a failure). A maximized, fullscreen window is never
    /// resized, and nor is a tiled one (`tiled.any()`): its size is the desktop's, and a request
    /// would only fight it (Preview and Photos do the same). Returns whether it asked for a size.
    pub(crate) fn loaded(
        &self,
        natural: Option<NaturalSize>,
        window: WindowState,
        tiled: Tiled,
    ) -> bool {
        if holds_its_size(window, tiled) {
            return false;
        }
        let screen = self.sizer.screen();
        let wanted = fitted(
            natural.map(|natural| logical_of(natural, screen)),
            work_for(screen, self.replaced),
            natural.map_or(LEAST, least_of),
        )
        .start();
        // The window already is that size: nothing to ask.
        if wanted == self.sizer.size() {
            return false;
        }
        self.sizer.request_size(wanted);
        true
    }
}

/// Whether the desktop, not the app, decides the window's size now.
fn holds_its_size(window: WindowState, tiled: Tiled) -> bool {
    window.maximized == Maximized::On || window.fullscreen == Fullscreen::On || tiled.any()
}

/// What to size a window to for `basis`: the document's own size, else what the header of `file`
/// says (a picture's or movie's size, an audio file's compact window), else nothing (the default
/// window). The header is read on a blocking thread, never on
/// the UI's: a video's takes a probe of the file.
pub(crate) async fn natural_for(basis: SizeBasis, file: Option<FilePath>) -> Option<NaturalSize> {
    natural_with(basis, file, natural_of).await
}

async fn natural_with(
    basis: SizeBasis,
    file: Option<FilePath>,
    probe: fn(&FilePath) -> Option<NaturalSize>,
) -> Option<NaturalSize> {
    match basis {
        SizeBasis::Natural(natural) => Some(natural),
        SizeBasis::Default => None,
        SizeBasis::Header => {
            let file = file?;
            let size = tokio::task::spawn_blocking(move || probe(&file)).await;
            size.ok().flatten()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dioxus::prelude::*;
    use ds::prelude::Scale;
    use ds::window::tiled::TileEdge;
    use ds_blitz::{ScreenOf, WorkBasis};
    use ds_harness::{Clock, Harness, HarnessConfig, Viewport, WindowScreen};

    /// A screen whose 85% is 1700 by 1020.
    const SCREEN: Extent = Extent::new(2000, 1200);

    fn start(size: WindowSize) -> (u32, u32) {
        (size.start().width, size.start().height)
    }

    fn of(width: u32, height: u32) -> (u32, u32) {
        start(fitted(Some(Extent::new(width, height)), SCREEN, LEAST))
    }

    fn output(physical: Extent, scale: Scale) -> ScreenArea {
        ScreenArea::new(physical, scale, ScreenOf::Window).unwrap()
    }

    #[test]
    fn no_natural_size_is_the_default_window_with_the_least() {
        assert_eq!(fitted(None, SCREEN, LEAST), WINDOW.with_least(480, 320));
        assert_eq!(
            fitted(Some(Extent::new(0, 40)), SCREEN, LEAST),
            WINDOW.with_least(480, 320),
            "an empty size is none"
        );
    }

    #[test]
    fn a_size_between_the_least_and_the_cap_is_taken_as_it_is() {
        assert_eq!(of(800, 600), (800, 600));
        assert_eq!(of(1700, 1020), (1700, 1020));
        assert_eq!(of(480, 320), (480, 320));
    }

    #[test]
    fn a_smaller_picture_gets_the_least_and_stays_centred() {
        assert_eq!(of(64, 64), (480, 320));
        assert_eq!(of(300, 900), (480, 900), "only the short axis is raised");
    }

    #[test]
    fn a_larger_picture_is_scaled_to_the_cap_keeping_its_ratio() {
        assert_eq!(of(3200, 1000), (1700, 531), "wider than the cap");
        assert_eq!(of(1000, 4000), (480, 1020), "taller than the cap, narrow");
        assert_eq!(of(4000, 3000), (1360, 1020), "the height limits");
        assert_eq!(of(6000, 3000), (1700, 850), "the width limits");
        assert_eq!(of(1800, 1000), (1700, 944), "rounded to whole pixels");
    }

    #[test]
    fn an_extreme_ratio_holds_the_short_axis_at_the_least() {
        assert_eq!(of(100_000, 10), (1700, 320));
        assert_eq!(of(10, 100_000), (480, 1020));
    }

    #[test]
    fn a_claim_of_a_hundred_thousand_pixels_each_way_just_meets_the_cap() {
        assert_eq!(of(100_000, 100_000), (1020, 1020));
        assert_eq!(of(u32::MAX, u32::MAX), (1020, 1020));
        assert_eq!(of(u32::MAX, 1), (1700, 320));
    }

    #[test]
    fn the_window_never_exceeds_the_cap_nor_falls_below_the_least() {
        for (w, h) in [
            (1, 1),
            (479, 319),
            (2000, 10),
            (10, 2000),
            (1701, 1021),
            (9999, 7),
        ] {
            let (width, height) = of(w, h);
            assert!((480..=1700).contains(&width), "{w}x{h} gave {width}");
            assert!((320..=1020).contains(&height), "{w}x{h} gave {height}");
        }
    }

    #[test]
    fn the_work_area_is_the_screen_less_the_top_bar_and_a_fixed_one_without() {
        let hd = output(Extent::new(1920, 1080), Scale(120));
        assert_eq!(work_for(Some(hd), None), Extent::new(1920, 1048));
        let retina = output(Extent::new(3840, 2160), Scale(240));
        assert_eq!(
            work_for(Some(retina), None),
            Extent::new(1920, 1048),
            "logical pixels, whatever the scale"
        );
        assert_eq!(
            work_for(None, None),
            DEFAULT_SCREEN,
            "before the event loop"
        );
        assert_eq!(
            work_for(Some(hd), Some(Extent::new(640, 480))),
            Extent::new(640, 480),
            "the override wins"
        );
        let less = hd.less(TOP_BAR);
        assert_eq!(less.basis, WorkBasis::LessReserve);
    }

    #[test]
    fn a_picture_opens_at_one_image_pixel_to_one_device_pixel() {
        let pixels = |w, h| {
            NaturalSize::Pixels(PixelSize {
                width: anyview_core::PixelLen(w),
                height: anyview_core::PixelLen(h),
            })
        };
        let on = |physical: Extent, scale: Scale, natural: NaturalSize| {
            let area = output(physical, scale);
            start(fitted(
                Some(logical_of(natural, Some(area))),
                work_for(Some(area), None),
                LEAST,
            ))
        };
        let (hd, retina) = (Extent::new(1920, 1080), Extent::new(3840, 2160));
        assert_eq!(on(hd, Scale(120), pixels(1200, 800)), (1200, 800));
        assert_eq!(on(retina, Scale(240), pixels(1200, 800)), (600, 400));
        assert_eq!(on(retina, Scale(240), pixels(2400, 1600)), (1200, 800));
        assert_eq!(
            on(retina, Scale(240), pixels(8000, 4000)),
            (1632, 816),
            "a large picture is fitted after it is halved"
        );
        let page = NaturalSize::Points(PixelSize {
            width: anyview_core::PixelLen(612),
            height: anyview_core::PixelLen(792),
        });
        assert_eq!(
            on(retina, Scale(240), page),
            (612, 792),
            "a point is a logical pixel, on any screen"
        );
        assert_eq!(logical_of(pixels(1200, 800), None), Extent::new(1200, 800));
    }

    #[test]
    fn an_audio_file_opens_a_small_window_below_a_pictures_least() {
        let compact = NaturalSize::Compact(audio_window_size());
        let wanted = fitted(Some(logical_of(compact, None)), SCREEN, least_of(compact));
        assert_eq!(start(wanted), (360, 460), "narrower than a picture's 480");
        let on_retina = output(Extent::new(3840, 2160), Scale(240));
        assert_eq!(
            logical_of(compact, Some(on_retina)),
            Extent::new(360, 460),
            "logical pixels on any screen"
        );
        assert_eq!(least_of(NaturalSize::Points(audio_window_size())), LEAST);
    }

    #[test]
    fn an_audio_file_is_found_by_its_header_and_a_picture_is_not_audio() {
        let dir = tempfile::tempdir().unwrap();
        let song = dir.path().join("song.mp3");
        std::fs::copy(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../anyview-peek/tests/fixtures/audio/plain.mp3"
            ),
            &song,
        )
        .unwrap();
        let song = FilePath::new(song).unwrap();
        assert_eq!(
            natural_of(&song),
            Some(NaturalSize::Compact(audio_window_size()))
        );
        let text = dir.path().join("a.txt");
        std::fs::write(&text, "hello").unwrap();
        assert_eq!(natural_of(&FilePath::new(text).unwrap()), None);
    }

    #[test]
    fn the_screen_is_set_as_width_x_height_and_anything_else_is_ignored() {
        assert_eq!(parse_screen("1280x800"), Some(Extent::new(1280, 800)));
        assert_eq!(parse_screen(" 640 X 480 "), Some(Extent::new(640, 480)));
        for bad in ["", "1280", "0x800", "1280x0", "-1x5", "axb", "1x2x3"] {
            assert_eq!(parse_screen(bad), None, "{bad:?}");
        }
    }

    /// A window of the harness's with a [`WindowFit`] on its sizer, and the harness to drive it.
    fn window(screen: WindowScreen, size: Extent) -> (Harness, WindowFit) {
        #[allow(non_snake_case)]
        fn Blank() -> Element {
            rsx! { div {} }
        }
        let config = HarnessConfig::new(Viewport {
            width: size.width,
            height: size.height,
            scale_percent: 100,
        })
        .with_clock(Clock::Virtual)
        .with_window_screen(screen);
        let harness = Harness::new(Blank, config);
        let fit = WindowFit::new(harness.window_sizer(), None);
        (harness, fit)
    }

    fn page() -> NaturalSize {
        NaturalSize::Points(PixelSize {
            width: anyview_core::PixelLen(612),
            height: anyview_core::PixelLen(792),
        })
    }

    fn area(logical_height: u32) -> WindowScreen {
        let physical = Extent::new(logical_height * 16 / 9, logical_height);
        WindowScreen::Area(output(physical, Scale(120)))
    }

    #[test]
    fn a_load_asks_for_the_size_of_its_content() {
        let (mut harness, fit) = window(area(1000), Extent::new(1000, 700));
        assert!(harness.within(|| fit.loaded(Some(page()), WindowState::default(), Tiled::NONE)));
        // The page fits the 85% of the work area, so it is its own size.
        assert_eq!(harness.window_requests(), vec![Extent::new(612, 792)]);
    }

    #[test]
    fn each_different_file_sizes_the_window_afresh() {
        let (mut harness, fit) = window(area(1000), Extent::new(1000, 700));
        assert!(harness.within(|| fit.loaded(Some(page()), WindowState::default(), Tiled::NONE)));
        let wide = NaturalSize::Points(PixelSize {
            width: anyview_core::PixelLen(800),
            height: anyview_core::PixelLen(400),
        });
        assert!(harness.within(|| fit.loaded(Some(wide), WindowState::default(), Tiled::NONE)));
        assert_eq!(
            harness.window_requests(),
            vec![Extent::new(612, 792), Extent::new(800, 400)]
        );
    }

    #[test]
    fn a_file_with_no_natural_size_takes_the_default_window() {
        let (mut harness, fit) = window(area(1000), Extent::new(612, 792));
        assert!(harness.within(|| fit.loaded(None, WindowState::default(), Tiled::NONE)));
        assert_eq!(harness.window_requests(), vec![Extent::new(1000, 700)]);
    }

    #[test]
    fn a_page_taller_than_the_screen_is_scaled_to_its_cap() {
        let (mut harness, fit) = window(area(900), Extent::new(1000, 700));
        assert!(harness.within(|| fit.loaded(Some(page()), WindowState::default(), Tiled::NONE)));
        // The work area is 900 less the 32 bar, 85% of it is 737: 612 by 792 scales to 570 by 737.
        assert_eq!(harness.window_requests(), vec![Extent::new(570, 737)]);
    }

    #[test]
    fn a_window_the_person_resized_follows_the_next_file_as_preview_does() {
        let (mut harness, fit) = window(area(1000), Extent::new(1000, 700));
        harness.resize_window(Extent::new(800, 500));
        assert!(harness.within(|| fit.loaded(Some(page()), WindowState::default(), Tiled::NONE)));
        assert_eq!(harness.window_requests(), vec![Extent::new(612, 792)]);
    }

    #[test]
    fn a_maximized_fullscreen_or_tiled_window_is_never_resized_by_a_new_file() {
        let maximized = WindowState {
            maximized: Maximized::On,
            ..WindowState::default()
        };
        let fullscreen = WindowState {
            fullscreen: Fullscreen::On,
            ..WindowState::default()
        };
        for (name, state, tiled) in [
            ("maximized", maximized, Tiled::NONE),
            ("fullscreen", fullscreen, Tiled::NONE),
            (
                "tiled on the left",
                WindowState::default(),
                Tiled::NONE.with(TileEdge::Left),
            ),
            (
                "tiled on all four sides",
                WindowState::default(),
                [
                    TileEdge::Top,
                    TileEdge::Bottom,
                    TileEdge::Left,
                    TileEdge::Right,
                ]
                .into_iter()
                .collect(),
            ),
        ] {
            let (mut harness, fit) = window(area(1000), Extent::new(1000, 700));
            assert!(
                !harness.within(|| fit.loaded(Some(page()), state, tiled)),
                "{name}"
            );
            assert!(harness.window_requests().is_empty(), "{name}");
            assert!(
                harness.within(|| fit.loaded(Some(page()), WindowState::default(), Tiled::NONE)),
                "{name}: the same window, restored, is sized again"
            );
        }
    }

    #[test]
    fn a_header_is_probed_off_the_thread_that_asked() {
        static PROBED_ON: std::sync::Mutex<Option<std::thread::ThreadId>> =
            std::sync::Mutex::new(None);
        fn size_of(_: &FilePath) -> Option<NaturalSize> {
            *PROBED_ON.lock().unwrap() = Some(std::thread::current().id());
            Some(NaturalSize::Pixels(PixelSize {
                width: anyview_core::PixelLen(640),
                height: anyview_core::PixelLen(360),
            }))
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let file = FilePath::new(std::env::current_exe().unwrap()).unwrap();
        let pixels = |w, h| {
            Some(NaturalSize::Pixels(PixelSize {
                width: anyview_core::PixelLen(w),
                height: anyview_core::PixelLen(h),
            }))
        };
        assert_eq!(
            runtime.block_on(natural_with(SizeBasis::Header, Some(file.clone()), size_of)),
            pixels(640, 360),
            "the header's size, read by the probe"
        );
        let probed_on = PROBED_ON.lock().unwrap().unwrap();
        assert_ne!(
            probed_on,
            std::thread::current().id(),
            "the probe did not run on the thread that asked"
        );
        assert_eq!(
            runtime.block_on(natural_with(
                SizeBasis::Default,
                Some(file.clone()),
                size_of
            )),
            None,
            "a failure is the default window, header or not"
        );
        assert_eq!(
            runtime.block_on(natural_with(SizeBasis::Header, None, size_of)),
            None,
            "no file, no header"
        );
        assert_eq!(
            runtime.block_on(natural_with(SizeBasis::Natural(page()), None, size_of)),
            Some(page()),
            "the document's own size needs no probe"
        );
    }

    #[test]
    fn a_window_already_that_size_is_not_asked() {
        let (mut harness, fit) = window(area(1000), Extent::new(612, 792));
        assert!(!harness.within(|| fit.loaded(Some(page()), WindowState::default(), Tiled::NONE)));
        assert!(harness.window_requests().is_empty());
    }
}
