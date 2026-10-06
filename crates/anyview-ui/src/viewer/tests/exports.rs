//! Opening the export sheet: what it opens on depends on the stage and, for a recording, on what
//! the host can write.

use super::support::*;
use crate::command::Command;
use crate::sheet::{ExportDraft, MediaOffer, Sheet, SheetParams};
use crate::stage::{Pace, Stage};
use crate::viewer::{Viewer, ViewerIn, ViewerParams};
use anyview_core::{
    AudioTarget, Fact, FactLabel, FactValue, FileAction, MediaExport, MediaExportKind,
};
use ds_core::machine::Machine;
use ds_core::time::stamp::Stamp;

fn needs() -> Fact {
    Fact {
        label: FactLabel::Needs,
        value: FactValue::text("anyview-ffmpeg (to convert it)"),
    }
}

/// The sheet after Export is asked of `stage` with `offer` on offer.
fn after_export(stage: Stage, offer: MediaOffer) -> Sheet {
    let params = ViewerParams {
        sheet: SheetParams {
            media: offer,
            ..SheetParams::default()
        },
        ..params()
    };
    let viewer = Viewer {
        stage,
        ..Viewer::default()
    };
    let (viewer, _) = viewer.step(
        ViewerIn::Run(Command::File(FileAction::Export)),
        Stamp(0),
        &(),
        &params,
    );
    viewer.sheet
}

#[test]
fn an_image_opens_the_sheet_on_its_default_whatever_is_offered() {
    assert_eq!(
        after_export(image(), MediaOffer::default()),
        Sheet::Export { draft: png() }
    );
}

#[test]
fn a_recording_opens_the_sheet_on_the_first_kind_on_offer() {
    let offer = MediaOffer::new(
        vec![MediaExportKind::ToMp3, MediaExportKind::ToWav],
        Some(needs()),
    );
    assert_eq!(
        after_export(media(Pace::Playing), offer),
        Sheet::Export {
            draft: ExportDraft::Media(MediaExport::AudioOnly(AudioTarget::Mp3(
                AudioTarget::default_bitrate()
            )))
        }
    );
}

#[test]
fn a_recording_with_nothing_on_offer_opens_the_sheet_that_names_the_package() {
    assert_eq!(
        after_export(media(Pace::Paused), MediaOffer::new(vec![], Some(needs()))),
        Sheet::Unavailable { needs: needs() }
    );
}

#[test]
fn a_recording_with_nothing_on_offer_and_nothing_to_say_opens_no_sheet() {
    assert_eq!(
        after_export(media(Pace::Paused), MediaOffer::default()),
        Sheet::Closed
    );
}
