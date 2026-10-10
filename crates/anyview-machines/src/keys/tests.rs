use super::*;
use crate::chrome::{ChromeIn, PinReason};
use crate::context::{ContextIn, ContextMenu, Spot};
use crate::edits::Rewind;
use crate::navigate::NavigateIn;
use crate::palette::{Palette, PaletteIn, PaletteIndex, PaletteMove, PaletteScope};
use crate::panel::{Panel, PanelIn, PanelTab};
use crate::sheet::{Sheet, SheetIn};
use crate::stage::{
    FindHits, MediaIn, MediaStage, PageView, PdfIn, PdfStage, RasterIn, RasterStage, Stage,
    StageIn, StageParams, TextStage, ZoomDir,
};
use crate::typed::TypedText;
use crate::{DesktopService, PlatformAbilities};
use anyview_core::{DocPoint, DocUnit, PageIndex, Permille};
use ds_core::vocab::ShortcutKey::{
    Char, Down, Enter, Escape, Left, PageDown, Right, Shift, Space, Tab, Up,
};
use ds_core::vocab::{Shortcut, ShortcutKey};

const OPEN_PALETTE: Palette = Palette::Open {
    query: TypedText::from_static(""),
    selection: PaletteIndex(0),
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

/// A press as a table writes it.
#[derive(Clone, Copy)]
enum In {
    /// A key with no command modifier.
    Plain(&'static [ShortcutKey]),
    /// An action the keymap resolved.
    Chord(Act),
}

impl In {
    fn press(self) -> Press {
        match self {
            In::Plain(keys) => Press::Key(Shortcut(keys.to_vec())),
            In::Chord(act) => Press::Act(act),
        }
    }
}

use In::{Chord, Plain};

/// Name, press, sheet, palette, panel, stage, route.
type Case = (&'static str, In, Sheet, Palette, Panel, Stage, Route);

const CASES: &[Case] = &[
    (
        "a sheet takes Enter before the palette behind it",
        Plain(&[Enter]),
        Sheet::ConfirmTrash,
        OPEN_PALETTE,
        Panel::Hidden,
        IMAGE,
        Route::Sheet(SheetIn::Confirm),
    ),
    (
        "a sheet takes Esc",
        Plain(&[Escape]),
        Sheet::ConfirmTrash,
        Palette::Closed,
        INFO,
        IMAGE,
        Route::Sheet(SheetIn::Cancel),
    ),
    (
        "a sheet swallows a global action",
        Chord(Act::Palette),
        Sheet::ConfirmTrash,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::Swallowed,
    ),
    (
        "a sheet swallows an arrow so the sequence does not move behind it",
        Plain(&[Right]),
        Sheet::ConfirmTrash,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::Swallowed,
    ),
    (
        "the palette takes the arrows",
        Plain(&[Down]),
        Sheet::Closed,
        OPEN_PALETTE,
        Panel::Hidden,
        IMAGE,
        Route::Palette(PaletteIn::Move(PaletteMove::Down)),
    ),
    (
        "the palette takes Up",
        Plain(&[Up]),
        Sheet::Closed,
        OPEN_PALETTE,
        Panel::Hidden,
        IMAGE,
        Route::Palette(PaletteIn::Move(PaletteMove::Up)),
    ),
    (
        "the palette takes Esc before the panel",
        Plain(&[Escape]),
        Sheet::Closed,
        OPEN_PALETTE,
        INFO,
        IMAGE,
        Route::Palette(PaletteIn::Close),
    ),
    (
        "the palette action closes an open palette",
        Chord(Act::Palette),
        Sheet::Closed,
        OPEN_PALETTE,
        Panel::Hidden,
        IMAGE,
        Route::Palette(PaletteIn::Close),
    ),
    (
        "typing in the palette is the field's, not a route",
        Plain(&[Char('a')]),
        Sheet::Closed,
        OPEN_PALETTE,
        Panel::Hidden,
        IMAGE,
        Route::Swallowed,
    ),
    (
        "the palette swallows close",
        Chord(Act::Close),
        Sheet::Closed,
        OPEN_PALETTE,
        Panel::Hidden,
        IMAGE,
        Route::Swallowed,
    ),
    (
        "the palette action opens the palette",
        Chord(Act::Palette),
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::OpenPalette,
    ),
    (
        "the info action shows the info tab",
        Chord(Act::Info),
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::Panel(PanelIn::Choose(PanelTab::Info)),
    ),
    (
        "the info action on another tab switches to info",
        Chord(Act::Info),
        Sheet::Closed,
        Palette::Closed,
        THUMBS,
        IMAGE,
        Route::Panel(PanelIn::Choose(PanelTab::Info)),
    ),
    (
        "the info action on the info tab closes the panel",
        Chord(Act::Info),
        Sheet::Closed,
        Palette::Closed,
        INFO,
        IMAGE,
        Route::Panel(PanelIn::Close),
    ),
    (
        "close closes the window",
        Chord(Act::Close),
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::CloseWindow,
    ),
    (
        "open chooses another file",
        Chord(Act::OpenFile),
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::OpenFile,
    ),
    (
        "Esc closes a find before the panel",
        Plain(&[Escape]),
        Sheet::Closed,
        Palette::Closed,
        INFO,
        FINDING_PDF,
        Route::Stage(StageIn::Pdf(PdfIn::CloseFind)),
    ),
    (
        "Esc closes the panel when the stage has nothing open",
        Plain(&[Escape]),
        Sheet::Closed,
        Palette::Closed,
        INFO,
        IMAGE,
        Route::Panel(PanelIn::Close),
    ),
    (
        "Esc with nothing open is a dismissal",
        Plain(&[Escape]),
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::Dismiss,
    ),
    (
        "a global action outranks the stage",
        Chord(Act::Close),
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        TEXT,
        Route::CloseWindow,
    ),
    (
        "plus zooms an image in",
        Plain(&[Char('+')]),
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
        Plain(&[Space]),
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        MEDIA,
        Route::Stage(StageIn::Media(MediaIn::Toggle)),
    ),
    (
        "shift right seeks media",
        Plain(&[Shift, Right]),
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        MEDIA,
        Route::Stage(StageIn::Media(MediaIn::SeekForward)),
    ),
    (
        "page down turns a pdf page",
        Plain(&[PageDown]),
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        PDF,
        Route::Stage(StageIn::Pdf(PdfIn::NextPage)),
    ),
    (
        "find opens the palette as a find in a text",
        Chord(Act::Find),
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        TEXT,
        Route::OpenFind,
    ),
    (
        "find opens the palette as a find in a pdf",
        Chord(Act::Find),
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        PDF,
        Route::OpenFind,
    ),
    (
        "find has nothing to find in a picture",
        Chord(Act::Find),
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::Ignored,
    ),
    (
        "find in the open palette makes what is typed a find",
        Chord(Act::Find),
        Sheet::Closed,
        OPEN_PALETTE,
        Panel::Hidden,
        TEXT,
        Route::Palette(PaletteIn::ToFind),
    ),
    (
        "find in the palette of a picture is swallowed",
        Chord(Act::Find),
        Sheet::Closed,
        OPEN_PALETTE,
        Panel::Hidden,
        IMAGE,
        Route::Swallowed,
    ),
    (
        "space on a still image holds the hand out",
        Plain(&[Space]),
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::Hand(crate::HandIn::SpaceDown),
    ),
    (
        "h on a still image is the pan tool",
        Plain(&[Char('h')]),
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::Hand(crate::HandIn::Toggle),
    ),
    (
        "c on a still image is the crop tool",
        Plain(&[Char('c')]),
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::Hand(crate::HandIn::Use(crate::Tool::Crop)),
    ),
    (
        "save on a picture saves its changes",
        Chord(Act::Save),
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::Save,
    ),
    (
        "save on a PDF is nothing to this window",
        Chord(Act::Save),
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        PDF,
        Route::Ignored,
    ),
    (
        "space on an animation plays it",
        Plain(&[Space]),
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        ANIMATION,
        Route::Stage(StageIn::Raster(RasterIn::TogglePlayback)),
    ),
    (
        "space on a pdf has no meaning and goes nowhere",
        Plain(&[Space]),
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        PDF,
        Route::Ignored,
    ),
    (
        "right walks the sequence",
        Plain(&[Right]),
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::Navigate(NavigateIn::Next),
    ),
    (
        "left walks the sequence over media too",
        Plain(&[Left]),
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        MEDIA,
        Route::Navigate(NavigateIn::Previous),
    ),
    (
        "tab moves focus into the chrome",
        Plain(&[Tab]),
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::Chrome(ChromeIn::Pin(PinReason::KeyboardFocus)),
    ),
    (
        "shift tab moves it out",
        Plain(&[Shift, Tab]),
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::Chrome(ChromeIn::Unpin(PinReason::KeyboardFocus)),
    ),
    (
        "undo takes back the last edit",
        Chord(Act::Undo),
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::Rewind(Rewind::Undo),
    ),
    (
        "redo does it again",
        Chord(Act::Redo),
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::Rewind(Rewind::Redo),
    ),
    (
        "a file action is for the window to run, not a region",
        Chord(Act::File(anyview_core::FileAction::Print)),
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::Ignored,
    ),
    (
        "zoom in by its action reaches the stage",
        Chord(Act::ZoomIn),
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
        "an unbound key goes nowhere",
        Plain(&[Char('q')]),
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
        Route::Ignored,
    ),
    (
        "no stage still routes the chords and the arrows",
        Plain(&[Right]),
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
            platform: PlatformAbilities::ALL,
            chords: Chords::Viewer,
        };
        let got = route(&keys.press(), regions);
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
        platform: PlatformAbilities::ALL,
        chords: Chords::Viewer,
    };
    const OPEN: ContextMenu = ContextMenu::Open {
        at: Spot { x: 1, y: 2 },
    };
    // name, the menu, the keys, where they go
    let cases: Vec<(&str, &'static ContextMenu, In, Route)> = vec![
        (
            "the menu key opens it",
            &ContextMenu::Closed,
            Plain(&[ShortcutKey::ContextMenu]),
            Route::OpenContextMenu,
        ),
        (
            "Escape closes it",
            &OPEN,
            Plain(&[Escape]),
            Route::Context(ContextIn::Close),
        ),
        (
            "the global chords wait while it is open",
            &OPEN,
            Chord(Act::Palette),
            Route::Swallowed,
        ),
        (
            "so do the arrows, which are the menu's own",
            &OPEN,
            Plain(&[Down]),
            Route::Swallowed,
        ),
    ];
    for (name, context, keys, want) in cases {
        let got = route(&keys.press(), regions(context));
        assert_eq!(got, want, "{name}");
    }
}

#[test]
fn open_is_an_action_only_where_the_platform_has_a_file_chooser() {
    let params = StageParams::default();
    let routed = |platform| {
        route(
            &Press::Act(Act::OpenFile),
            Regions {
                sheet: &Sheet::Closed,
                palette: &Palette::Closed,
                context: &ContextMenu::Closed,
                panel: &Panel::Hidden,
                stage: &IMAGE,
                stage_params: &params,
                platform,
                chords: Chords::Viewer,
            },
        )
    };
    assert_eq!(routed(PlatformAbilities::ALL), Route::OpenFile);
    assert_ne!(
        routed(PlatformAbilities::ALL.without(DesktopService::FileChooser)),
        Route::OpenFile,
        "unbound without a chooser"
    );
}

#[test]
fn a_hosted_viewer_hears_no_chord_and_its_plain_keys_still_route() {
    let params = StageParams::default();
    let routed = |keys: In| {
        route(
            &keys.press(),
            Regions {
                sheet: &Sheet::Closed,
                palette: &Palette::Closed,
                context: &ContextMenu::Closed,
                panel: &Panel::Hidden,
                stage: &IMAGE,
                stage_params: &params,
                platform: PlatformAbilities::ALL,
                chords: Chords::None,
            },
        )
    };
    // name, the keys, where they go
    let cases: Vec<(&str, In, Route)> = vec![
        ("the palette's chord", Chord(Act::Palette), Route::Swallowed),
        ("Info", Chord(Act::Info), Route::Swallowed),
        ("Open", Chord(Act::OpenFile), Route::Swallowed),
        ("Close", Chord(Act::Close), Route::Swallowed),
        (
            "an arrow still walks the sequence",
            Plain(&[Right]),
            Route::Navigate(NavigateIn::Next),
        ),
        ("Esc has nothing to undo", Plain(&[Escape]), Route::Dismiss),
    ];
    for (name, keys, want) in cases {
        assert_eq!(routed(keys), want, "{name}");
    }
}
