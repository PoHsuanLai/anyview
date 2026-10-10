use crate::chrome::{Chrome, ChromeOut, PinReason, PinReasons};
use crate::command::{Command, StageCommand};
use crate::palette::{Palette, PaletteIndex, PaletteParams, PaletteScope};
use crate::panel::{PanelParams, PanelTab, PanelTabs};
use crate::picture::{PictureEditIn, PictureEditing, PictureEdits};
use crate::presentation::{ContentClass, PresentationParams};
use crate::sheet::ExportDraft;
use crate::stage::{MediaStage, Pace, RasterStage, Stage};
use crate::typed::TypedText;
use crate::viewer::{ViewerOut, ViewerParams};
use anyview_core::{
    DocPoint, FileAction, MediaLength, MediaTime, PixelLen, PixelSize, RasterExport, RasterTarget,
    Resize, Resume, Zoom,
};
use ds_core::vocab::Shown;
use std::time::Duration;

pub(super) const QUICK: Duration = Duration::from_millis(150);
pub(super) const FADE_IN: ViewerOut = ViewerOut::Chrome(ChromeOut::Fade {
    to: Shown::Visible,
    over: QUICK,
});

pub(super) fn params() -> ViewerParams {
    ViewerParams {
        panel: PanelParams {
            tabs: PanelTabs::of(&[PanelTab::Info, PanelTab::Tracks]),
        },
        palette: PaletteParams {
            rows: vec![
                Command::File(FileAction::Export),
                Command::File(FileAction::MoveToTrash),
                Command::Stage(StageCommand::TogglePlayback),
                Command::File(FileAction::RotateRight),
                Command::File(FileAction::PlayInMiniWindow),
                Command::File(FileAction::Share),
            ],
        },
        presentation: PresentationParams {
            content: ContentClass::Media,
        },
        ..ViewerParams::default()
    }
}

pub(super) fn image() -> Stage {
    Stage::Raster(RasterStage::default())
}

/// A picture that has landed and can be saved with changes: 400 by 300 pixels.
pub(super) fn editable() -> PictureEdits {
    PictureEdits::default().step(PictureEditIn::Landed {
        size: Some(PixelSize {
            width: PixelLen(400),
            height: PixelLen(300),
        }),
        editing: PictureEditing::Allowed,
    })
}

/// What the stage says when it is brought back to fitting the window after the picture changed.
pub(super) fn refitted() -> ViewerOut {
    ViewerOut::Stage(crate::stage::StageOut::Raster(
        crate::stage::RasterOut::Remember(Resume::Raster {
            zoom: Zoom::Fit,
            centre: DocPoint::default(),
        }),
    ))
}

pub(super) fn length() -> MediaLength {
    MediaLength(MediaTime::from_secs(60))
}

pub(super) fn media(paused: Pace) -> Stage {
    let at = MediaTime::from_secs(5);
    let length = length();
    Stage::Media(match paused {
        Pace::Playing => MediaStage::Playing { at, length },
        Pace::Paused => MediaStage::Paused { at, length },
    })
}

pub(super) fn palette_on(row: usize) -> Palette {
    Palette::Open {
        query: TypedText::EMPTY,
        selection: PaletteIndex(row),
        scope: PaletteScope::Commands,
    }
}

pub(super) fn menu_pinned() -> Chrome {
    Chrome::Pinned {
        by: PinReasons::of(PinReason::MenuOpen),
    }
}

pub(super) fn png() -> ExportDraft {
    ExportDraft::Raster(RasterExport::Image(
        RasterTarget::Png,
        Resize::Original,
        anyview_core::MetadataCarry::default(),
    ))
}
