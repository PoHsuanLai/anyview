//! The options of a recording's export: the stretch kept and the bitrate.

use super::parts::segments;
use crate::{ExportDraft, ExportFacts, ExportOption, TrimSpan};
use anyview_core::{AudioTarget, Bitrate, MediaExport};
use dioxus::prelude::*;
use ds::components::content::text_runs::TextLine;
use ds::components::fields::field_row::{FieldRow, RowLayout};

/// The bitrates on offer, in kilobits a second.
const BITRATES: [u32; 5] = [96, 128, 192, 256, 320];
pub(super) fn range(
    draft: ExportDraft,
    facts: ExportFacts,
    ontune: EventHandler<ExportOption>,
) -> Element {
    let current = match draft {
        ExportDraft::Media(MediaExport::Trim(range)) if Some(range) == facts.marks => {
            TrimSpan::Marks
        }
        ExportDraft::Raster(_)
        | ExportDraft::Pdf(_)
        | ExportDraft::Text(_)
        | ExportDraft::Media(_) => TrimSpan::Whole,
    };
    rsx! {
        FieldRow { label: "Range", layout: RowLayout::Form,
            {segments(
                "Range",
                &[TrimSpan::Whole, TrimSpan::Marks],
                |span| match span {
                    TrimSpan::Whole => "Whole".to_owned(),
                    TrimSpan::Marks => "Trim marks".to_owned(),
                },
                Some(current),
                EventHandler::new(move |span| ontune.call(ExportOption::Trim(span))),
            )}
        }
    }
}
pub(super) fn bitrate(draft: ExportDraft, ontune: EventHandler<ExportOption>) -> Element {
    let current = match draft {
        ExportDraft::Media(MediaExport::AudioOnly(
            AudioTarget::M4a(rate) | AudioTarget::Mp3(rate) | AudioTarget::Opus(rate),
        )) => rate.kbps(),
        ExportDraft::Raster(_)
        | ExportDraft::Pdf(_)
        | ExportDraft::Text(_)
        | ExportDraft::Media(_) => AudioTarget::default_bitrate().kbps(),
    };
    rsx! {
        FieldRow {
            label: "Bitrate",
            help: Some(TextLine::from("Kilobits a second")),
            layout: RowLayout::Form,
            {segments(
                "Bitrate",
                &BITRATES,
                |kbps| kbps.to_string(),
                Some(current),
                EventHandler::new(move |kbps: u32| ontune.call(ExportOption::Bitrate(Bitrate::from_kbps(kbps)))),
            )}
        }
    }
}
