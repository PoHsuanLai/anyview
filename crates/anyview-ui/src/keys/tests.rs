use super::*;
use crate::chrome::{ChromeIn, PinReason};
use crate::navigate::NavigateIn;
use crate::palette::{Palette, PaletteIn, PaletteMove, RowIndex};
use crate::panel::{Panel, PanelIn, PanelTab};
use crate::sheet::{Sheet, SheetIn};
use crate::stage::{
    FindHits, MediaIn, MediaStage, PageView, PdfIn, PdfStage, RasterIn, RasterStage, Stage,
    StageIn, StageParams, TextIn, TextStage, ZoomDir,
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
        "command f opens a text find",
        &[Super, Char('f')],
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        TEXT,
        Route::Stage(StageIn::Text(TextIn::Find(TypedText::EMPTY))),
    ),
    (
        "space on an image is for its animation",
        &[Space],
        Sheet::Closed,
        Palette::Closed,
        Panel::Hidden,
        IMAGE,
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
            panel,
            stage,
            stage_params: &params,
        };
        let got = route(&Shortcut(keys.to_vec()), regions);
        assert_eq!(&got, want, "{name}");
    }
}
