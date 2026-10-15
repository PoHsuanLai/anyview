//! The options of a picture or a document's image: quality, size, metadata, format, resolution.

use super::parts::{segments, stepper};
use crate::{
    ExportDraft, ExportFacts, ExportKindPick, ExportOption, MAX_LONG_EDGE, SizePick, format_of,
    kind_name, quality_of,
};
use anyview_core::{
    Dpi, MediaExport, MetadataCarry, PdfExport, Percent, PixelLen, RasterExport, RasterExportKind,
    Resize,
};
use dioxus::prelude::*;
use ds::components::content::text_runs::TextLine;
use ds::components::fields::field_row::{FieldRow, RowLayout};
use ds::prelude::{Fraction, Slider};

/// The resolutions on offer for page images.
const RESOLUTIONS: [Dpi; 3] = [Dpi::PAGE, Dpi::SCREEN, Dpi::PRINT];
/// The image formats page images come in.
const PAGE_FORMATS: [RasterExportKind; 5] = [
    RasterExportKind::Png,
    RasterExportKind::Jpeg,
    RasterExportKind::Webp,
    RasterExportKind::Avif,
    RasterExportKind::Tiff,
];
pub(super) fn quality(draft: ExportDraft, ontune: EventHandler<ExportOption>) -> Element {
    let percent = match draft {
        ExportDraft::Raster(
            RasterExport::Image(target, ..) | RasterExport::Adjusted(target, _),
        )
        | ExportDraft::Pdf(PdfExport::PageImages(_, target, _))
        | ExportDraft::Media(MediaExport::CurrentFrame(target)) => {
            quality_of(target).unwrap_or(Percent(100))
        }
        ExportDraft::Raster(RasterExport::Pdf)
        | ExportDraft::Pdf(_)
        | ExportDraft::Text(_)
        | ExportDraft::Media(_) => Percent(100),
    };
    rsx! {
        FieldRow { label: "Quality", layout: RowLayout::Form,
            div { class: "viewer-export-slider",
                Slider {
                    label: "Quality",
                    value: Fraction(percent.0.saturating_mul(10)),
                    step: Fraction(10),
                    onchange: move |to: Fraction| ontune.call(ExportOption::Quality(Percent(to.0 / 10))),
                }
                span { class: "viewer-export-readout", "{percent.0}%" }
            }
        }
    }
}
pub(super) fn size(
    draft: ExportDraft,
    facts: ExportFacts,
    ontune: EventHandler<ExportOption>,
) -> Element {
    let resize = match draft {
        ExportDraft::Raster(RasterExport::Image(_, resize, _)) => resize,
        ExportDraft::Raster(RasterExport::Adjusted(..) | RasterExport::Pdf)
        | ExportDraft::Pdf(_)
        | ExportDraft::Text(_)
        | ExportDraft::Media(_) => Resize::Original,
    };
    let long = match resize {
        Resize::LongEdge(edge) => edge,
        Resize::Original | Resize::Scaled(_) => facts
            .image
            .map_or(PixelLen(1920), |size| {
                PixelLen(size.width.0.max(size.height.0))
            })
            .min(MAX_LONG_EDGE),
    };
    let edge = i32::try_from(long.0).unwrap_or(i32::MAX);
    rsx! {
        FieldRow { label: "Size", layout: RowLayout::Form,
            div { class: "viewer-export-stack",
                {segments(
                    "Size",
                    &[SizePick::Original, SizePick::Half, SizePick::Quarter, SizePick::LongEdge],
                    |pick| match pick {
                        SizePick::Original => "Original".to_owned(),
                        SizePick::Half => "50%".to_owned(),
                        SizePick::Quarter => "25%".to_owned(),
                        SizePick::LongEdge => "Long edge".to_owned(),
                    },
                    SizePick::of(resize),
                    EventHandler::new(move |pick: SizePick| ontune.call(ExportOption::Resize(pick.resize(long)))),
                )}
                if matches!(resize, Resize::LongEdge(_)) {
                    div { class: "viewer-export-slider",
                        {stepper(
                            "Long edge in pixels",
                            edge,
                            1,
                            i32::try_from(MAX_LONG_EDGE.0).unwrap_or(i32::MAX),
                            EventHandler::new(move |to: i32| ontune.call(ExportOption::Resize(
                                Resize::LongEdge(PixelLen(u32::try_from(to).unwrap_or(1))),
                            ))),
                        )}
                        span { class: "viewer-export-readout", "px" }
                    }
                }
            }
        }
    }
}
/// How much metadata the picture keeps: everything, all but where it was taken, or nothing.
pub(super) fn metadata(draft: ExportDraft, ontune: EventHandler<ExportOption>) -> Element {
    let keep = match draft {
        ExportDraft::Raster(RasterExport::Image(_, _, keep)) => keep,
        ExportDraft::Raster(RasterExport::Adjusted(..) | RasterExport::Pdf)
        | ExportDraft::Pdf(_)
        | ExportDraft::Text(_)
        | ExportDraft::Media(_) => MetadataCarry::default(),
    };
    rsx! {
        FieldRow { label: "Metadata", layout: RowLayout::Form,
            {segments(
                "Metadata",
                &[MetadataCarry::Keep, MetadataCarry::StripLocation, MetadataCarry::Drop],
                |keep| match keep {
                    MetadataCarry::Keep => "All".to_owned(),
                    MetadataCarry::StripLocation => "No location".to_owned(),
                    MetadataCarry::Drop => "None".to_owned(),
                },
                Some(keep),
                EventHandler::new(move |keep| ontune.call(ExportOption::Metadata(keep))),
            )}
        }
    }
}
pub(super) fn image_format(draft: ExportDraft, ontune: EventHandler<ExportOption>) -> Element {
    let current = match draft {
        ExportDraft::Pdf(PdfExport::PageImages(_, target, _)) => format_of(target),
        ExportDraft::Raster(_)
        | ExportDraft::Pdf(_)
        | ExportDraft::Text(_)
        | ExportDraft::Media(_) => RasterExportKind::Png,
    };
    rsx! {
        FieldRow { label: "Format", layout: RowLayout::Form,
            {segments(
                "Image format",
                &PAGE_FORMATS,
                |kind| kind_name(ExportKindPick::Raster(*kind)).to_owned(),
                Some(current),
                EventHandler::new(move |kind| ontune.call(ExportOption::ImageFormat(kind))),
            )}
        }
    }
}
pub(super) fn resolution(draft: ExportDraft, ontune: EventHandler<ExportOption>) -> Element {
    let current = match draft {
        ExportDraft::Pdf(PdfExport::PageImages(_, _, dpi)) => dpi,
        ExportDraft::Raster(_)
        | ExportDraft::Pdf(_)
        | ExportDraft::Text(_)
        | ExportDraft::Media(_) => Dpi::SCREEN,
    };
    rsx! {
        FieldRow {
            label: "Resolution",
            help: Some(TextLine::from("Dots per inch")),
            layout: RowLayout::Form,
            {segments(
                "Resolution",
                &RESOLUTIONS,
                |dpi| dpi.get().to_string(),
                Some(current),
                EventHandler::new(move |dpi| ontune.call(ExportOption::Resolution(dpi))),
            )}
        }
    }
}
