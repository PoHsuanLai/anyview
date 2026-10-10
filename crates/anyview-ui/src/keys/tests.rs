use super::*;
use crate::chrome::{ChromeIn, PinReason};
use crate::context::{ContextIn, ContextMenu, Spot};
use crate::navigate::NavigateIn;
use crate::palette::{Palette, PaletteIn, PaletteMove, PaletteScope, RowIndex};
use crate::panel::{Panel, PanelIn, PanelTab};
use crate::sheet::{Sheet, SheetIn};
use crate::stage::{
    FindHits, MediaIn, MediaStage, PageView, PdfIn, PdfStage, RasterIn, RasterStage, Stage,
    StageIn, StageParams, TextStage, ZoomDir,
};
use crate::typed::TypedText;
use anyview_core::{DocPoint, DocUnit, PageIndex, Permille};
use ds_core::vocab::ShortcutKey::{
    Char, Down, Enter, Escape, Left, PageDown, Right, Shift, Space, Super, Tab, Up,
};
use ds_core::vocab::{Shortcut, ShortcutKey};

const OPEN_PALETTE: Palette = Palette::Open {
    query: TypedText::from_static(""),
    selection: RowIndex(0),
    scope: PaletteScope::Commands,
};
const INFO: Panel = Panel::Shown {
    tab: PanelTab::Info,
};
const THUMBS: Panel = Panel::Shown {
    tab: PanelTab::Thumbnails,
};
const IMAGE: Stage = Stage::Raster(RasterStage::Fitted {
    turn: anyview_core::QuarterTurn::None,
    anim: crate::stage::Animation::Still,
});
const ANIMATION: Stage = Stage::Raster(RasterStage::Fitted {
    turn: anyview_core::QuarterTurn::None,
    anim: crate::stage::Animation::Paused {
        frame: crate::stage::FrameIndex(0),
        of: crate::stage::FrameCount(std::num::NonZeroU32::MIN),
        run: 0,
    },
});
const CENTRE: DocPoint = DocPoint {
    x: DocUnit(0),
    y: DocUnit(0),
};
const FINDING_PDF: Stage = Stage::Pdf(PdfStage::Finding {
    query: TypedText::from_static("cat"),
    hits: FindHits::Pending,
    view: PageView {
        page: PageIndex(0),
        offset: Permille(0),
        zoom: anyview_core::Zoom::Fit,
    },
});
const PDF: Stage = Stage::Pdf(PdfStage::Reading {
    view: PageView {
        page: PageIndex(0),
        offset: Permille(0),
        zoom: anyview_core::Zoom::Fit,
    },
});
const MEDIA: Stage = Stage::Media(MediaStage::Opening);
const TEXT: Stage = Stage::Text(TextStage::Reading {
    place: crate::stage::TextPlace {
        line: anyview_core::LineIndex(0),
        wrap: crate::stage::Wrap::On,
        view: crate::stage::TextView::Rendered,
    },
});

/// Name, keys, sheet, palette, panel, stage, route.
type Case = (
    &'static str,
    &'static [ShortcutKey],
    Sheet,
    Palette,
    Panel,
    Stage,
    Route,
);

const CASES: &[Case] = &[
    (
        "a sheet takes Enter before the palette behind it",
        &[Enter],
        Sheet::ConfirmTrash,
        OPEN_PALETTE,
        Panel::Hidden,
        IMAGE,
        Route::Sheet(SheetIn::Confirm),
    ),
    (
        "a sheet takes Esc",
        &[Escape],
        Sheet::ConfirmTrash,
        Palette::Closed,
        INFO,
        IMAGE,
        Route::Sheet(SheetIn::Cancel),
    ),
    (
        "a sheet swallows a global chord",
        &[Super, Char('k')],
        Sheet::ConfirmTrash,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::Swallowed,
    ),
    (
        "a sheet swallows an arrow so the sequence does not move behind it",
        &[Right],
        Sheet::ConfirmTrash,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::Swallowed,
    ),
    (
        "the palette takes the arrows",
        &[Down],
        Sheet::Closed,
        OPEN_PALETTE,
        Panel::Hidden,
        IMAGE,
        Route::Palette(PaletteIn::Move(PaletteMove::Down)),
    ),
    (
        "the palette takes Up",
        &[Up],
        Sheet::Closed,
        OPEN_PALETTE,
        Panel::Hidden,
        IMAGE,
        Route::Palette(PaletteIn::Move(PaletteMove::Up)),
    ),
    (
        "the palette takes Esc before the panel",
        &[Escape],
        Sheet::Closed,
        OPEN_PALETTE,
        INFO,
        IMAGE,
        Route::Palette(PaletteIn::Close),
    ),
    (
        "command k closes an open palette",
        &[Super, Char('k')],
        Sheet::Closed,
        OPEN_PALETTE,
        Panel::Hidden,
        IMAGE,
        Route::Palette(PaletteIn::Close),
    ),
    (
        "typing in the palette is the field's, not a route",
        &[Char('a')],
        Sheet::Closed,
        OPEN_PALETTE,
        Panel::Hidden,
        IMAGE,
        Route::Swallowed,
    ),
    (
        "the palette swallows command w",
        &[Super, Char('w')],
        Sheet::Closed,
        OPEN_PALETTE,
        Panel::Hidden,
        IMAGE,
        Route::Swallowed,
    ),
    (
        "command k opens the palette",
        &[Super, Char('k')],
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::OpenPalette,
    ),
    (
        "modifiers in any order are the same chord",
        &[Char('k'), Super],
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::OpenPalette,
    ),
    (
        "command i shows the info tab",
        &[Super, Char('i')],
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::Panel(PanelIn::Choose(PanelTab::Info)),
    ),
    (
        "command i on another tab switches to info",
        &[Super, Char('i')],
        Sheet::Closed,
        Palette::Closed,
        THUMBS,
        IMAGE,
        Route::Panel(PanelIn::Choose(PanelTab::Info)),
    ),
    (
        "command i on the info tab closes the panel",
        &[Super, Char('i')],
        Sheet::Closed,
        Palette::Closed,
        INFO,
        IMAGE,
        Route::Panel(PanelIn::Close),
    ),
    (
        "command w closes the window",
        &[Super, Char('w')],
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::CloseWindow,
    ),
    (
        "command o chooses another file",
        &[Super, Char('o')],
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::OpenFile,
    ),
    (
        "Esc closes a find before the panel",
        &[Escape],
        Sheet::Closed,
        Palette::Closed,
        INFO,
        FINDING_PDF,
        Route::Stage(StageIn::Pdf(PdfIn::CloseFind)),
    ),
    (
        "Esc closes the panel when the stage has nothing open",
        &[Escape],
        Sheet::Closed,
        Palette::Closed,
        INFO,
        IMAGE,
        Route::Panel(PanelIn::Close),
    ),
    (
        "Esc with nothing open is a dismissal",
        &[Escape],
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::Dismiss,
    ),
    (
        "a global chord outranks the stage",
        &[Super, Char('w')],
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        TEXT,
        Route::CloseWindow,
    ),
    (
        "plus zooms an image in",
        &[Char('+')],
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::Stage(StageIn::Raster(RasterIn::ZoomStep {
            dir: ZoomDir::In,
            at: CENTRE,
        })),
    ),
    (
        "space plays media",
        &[Space],
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        MEDIA,
        Route::Stage(StageIn::Media(MediaIn::Toggle)),
    ),
    (
        "shift right seeks media",
        &[Shift, Right],
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        MEDIA,
        Route::Stage(StageIn::Media(MediaIn::SeekForward)),
    ),
    (
        "page down turns a pdf page",
        &[PageDown],
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        PDF,
        Route::Stage(StageIn::Pdf(PdfIn::NextPage)),
    ),
    (
        "command f opens the palette as a find in a text",
        &[Super, Char('f')],
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        TEXT,
        Route::OpenFind,
    ),
    (
        "command f opens the palette as a find in a pdf",
        &[Super, Char('f')],
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        PDF,
        Route::OpenFind,
    ),
    (
        "command f has nothing to find in a picture",
        &[Super, Char('f')],
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::Ignored,
    ),
    (
        "command f in the open palette makes what is typed a find",
        &[Super, Char('f')],
        Sheet::Closed,
        OPEN_PALETTE,
        Panel::Hidden,
        TEXT,
        Route::Palette(PaletteIn::ToFind),
    ),
    (
        "command f in the palette of a picture is swallowed",
        &[Super, Char('f')],
        Sheet::Closed,
        OPEN_PALETTE,
        Panel::Hidden,
        IMAGE,
        Route::Swallowed,
    ),
    (
        "space on a still image holds the hand out",
        &[Space],
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::Hand(crate::HandIn::SpaceDown),
    ),
    (
        "h on a still image is the pan tool",
        &[Char('h')],
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::Hand(crate::HandIn::Toggle),
    ),
    (
        "space on an animation plays it",
        &[Space],
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        ANIMATION,
        Route::Stage(StageIn::Raster(RasterIn::TogglePlayback)),
    ),
    (
        "space on a pdf has no meaning and goes nowhere",
        &[Space],
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        PDF,
        Route::Ignored,
    ),
    (
        "right walks the sequence",
        &[Right],
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::Navigate(NavigateIn::Next),
    ),
    (
        "left walks the sequence over media too",
        &[Left],
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        MEDIA,
        Route::Navigate(NavigateIn::Previous),
    ),
    (
        "tab moves focus into the chrome",
        &[Tab],
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::Chrome(ChromeIn::Pin(PinReason::KeyboardFocus)),
    ),
    (
        "shift tab moves it out",
        &[Shift, Tab],
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::Chrome(ChromeIn::Unpin(PinReason::KeyboardFocus)),
    ),
    (
        "an unbound key goes nowhere",
        &[Char('q')],
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::Ignored,
    ),
    (
        "no stage still routes the chords and the arrows",
        &[Right],
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        Stage::NoStage,
        Route::Navigate(NavigateIn::Next),
    ),
];

#[test]
fn every_row_of_the_table_routes_as_written() {
    let params = StageParams::default();
    for (name, keys, sheet, palette, panel, stage, want) in CASES {
        let regions = Regions {
            sheet,
            palette,
            context: &ContextMenu::Closed,
            panel,
            stage,
            stage_params: &params,
            pick_files: true,
        };
        let got = route(&Shortcut(keys.to_vec()), regions);
        assert_eq!(&got, want, "{name}");
    }
}

#[test]
fn the_menu_key_and_shift_f10_open_the_context_menu_and_an_open_menu_takes_escape() {
    let params = StageParams::default();
    let regions = |context: &'static ContextMenu| Regions {
        sheet: &Sheet::Closed,
        palette: &Palette::Closed,
        context,
        panel: &Panel::Hidden,
        stage: &IMAGE,
        stage_params: &params,
        pick_files: true,
    };
    const OPEN: ContextMenu = ContextMenu::Open {
        at: Spot { x: 1, y: 2 },
    };
    // name, the menu, the keys, where they go
    let cases: Vec<(&str, &'static ContextMenu, Vec<ShortcutKey>, Route)> = vec![
        (
            "the menu key opens it",
            &ContextMenu::Closed,
            vec![ShortcutKey::ContextMenu],
            Route::OpenContextMenu,
        ),
        (
            "Escape closes it",
            &OPEN,
            vec![Escape],
            Route::Context(ContextIn::Close),
        ),
        (
            "the global chords wait while it is open",
            &OPEN,
            vec![Super, Char('k')],
            Route::Swallowed,
        ),
        (
            "so do the arrows, which are the menu's own",
            &OPEN,
            vec![Down],
            Route::Swallowed,
        ),
    ];
    for (name, context, keys, want) in cases {
        let got = route(&Shortcut(keys), regions(context));
        assert_eq!(got, want, "{name}");
    }
}

#[test]
fn command_o_is_a_chord_only_where_the_platform_has_a_file_chooser() {
    let params = StageParams::default();
    let routed = |pick_files| {
        route(
            &Shortcut(vec![ShortcutKey::Super, ShortcutKey::Char('o')]),
            Regions {
                sheet: &Sheet::Closed,
                palette: &Palette::Closed,
                context: &ContextMenu::Closed,
                panel: &Panel::Hidden,
                stage: &IMAGE,
                stage_params: &params,
                pick_files,
            },
        )
    };
    assert_eq!(routed(true), Route::OpenFile);
    assert_ne!(routed(false), Route::OpenFile, "unbound without a chooser");
}
