//! The rows of the export dialog's options: one function for each option, drawing the control
//! that suits its choices and reporting the person's pick as an [`ExportOption`].

use crate::ExportKindPick;
use crate::{
    ExportControl, ExportDraft, ExportFacts, ExportOption, MAX_LONG_EDGE, PageSpan, SizePick,
    TrimSpan, format_of, kind_name, quality_of,
};
use anyview_core::{
    AudioTarget, Bitrate, Dpi, MediaExport, MetadataCarry, Orientation, PageIndex, PageSelection,
    PaperSize, PdfExport, Percent, PixelLen, RasterExport, RasterExportKind, Resize, TextExport,
};
use dioxus::prelude::*;
use ds::components::content::text_runs::TextLine;
use ds::components::controls::segmented::Tracking;
use ds::components::fields::field_row::{FieldRow, RowLayout};
use ds::components::fields::stepper::model::StepRange;
use ds::components::fields::stepper::view::Stepper;
use ds::prelude::{Choice, Fraction, SegmentedControl, Slider};
use ds_core::word::Word;

/// The bitrates on offer, in kilobits a second.
const BITRATES: [u32; 5] = [96, 128, 192, 256, 320];

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

/// A segmented control that picks one of `values`, each said by `say`. With no `current` (a value
/// the control has no segment for) none is lit.
fn segments<T: Clone + PartialEq + 'static>(
    label: &'static str,
    values: &[T],
    say: impl Fn(&T) -> String,
    current: Option<T>,
    onchange: EventHandler<T>,
) -> Element {
    let choices: Vec<Choice<T>> = values
        .iter()
        .map(|value| Choice::new(value.clone(), say(value)))
        .collect();
    let tracking = match current {
        Some(current) => Tracking::SelectOne(current),
        None => Tracking::SelectAny(Vec::new()),
    };
    rsx! {
        SegmentedControl::<T> {
            label,
            choices,
            tracking,
            onchange: move |value: T| onchange.call(value),
        }
    }
}

/// A stepper of whole numbers over `min..=max`.
fn stepper(
    label: &'static str,
    value: i32,
    min: i32,
    max: i32,
    onchange: EventHandler<i32>,
) -> Element {
    rsx! {
        Stepper {
            label,
            value,
            range: StepRange::new(min, max, 1),
            onchange: move |to: i32| onchange.call(to),
        }
    }
}

/// The page number (from 1) of a page index.
fn number(page: PageIndex) -> i32 {
    i32::try_from(page.0.saturating_add(1)).unwrap_or(i32::MAX)
}

/// The pages the draft's page choice covers.
fn pages_of(draft: ExportDraft) -> Option<PageSelection> {
    match draft {
        ExportDraft::Pdf(PdfExport::Pdf(pages) | PdfExport::PageImages(pages, ..)) => Some(pages),
        ExportDraft::Raster(_)
        | ExportDraft::Pdf(PdfExport::PlainText | PdfExport::Markdown)
        | ExportDraft::Text(_)
        | ExportDraft::Media(_) => None,
    }
}

/// One option of the export, as a row of the form.
pub(super) fn option_row(
    control: ExportControl,
    draft: ExportDraft,
    span: PageSpan,
    facts: ExportFacts,
    ontune: EventHandler<ExportOption>,
) -> Element {
    match control {
        ExportControl::Quality => quality(draft, ontune),
        ExportControl::Size => size(draft, facts, ontune),
        ExportControl::Metadata => metadata(draft, ontune),
        ExportControl::Pages => pages(span, ontune),
        ExportControl::PageRange => page_range(draft, facts, ontune),
        ExportControl::ImageFormat => image_format(draft, ontune),
        ExportControl::Resolution => resolution(draft, ontune),
        ExportControl::Paper => paper(draft, ontune),
        ExportControl::Orientation => orientation(draft, ontune),
        ExportControl::Range => range(draft, facts, ontune),
        ExportControl::Bitrate => bitrate(draft, ontune),
    }
}

fn quality(draft: ExportDraft, ontune: EventHandler<ExportOption>) -> Element {
    let percent = match draft {
        ExportDraft::Raster(RasterExport::Image(target, ..))
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

fn size(draft: ExportDraft, facts: ExportFacts, ontune: EventHandler<ExportOption>) -> Element {
    let resize = match draft {
        ExportDraft::Raster(RasterExport::Image(_, resize, _)) => resize,
        ExportDraft::Raster(RasterExport::Pdf)
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
fn metadata(draft: ExportDraft, ontune: EventHandler<ExportOption>) -> Element {
    let keep = match draft {
        ExportDraft::Raster(RasterExport::Image(_, _, keep)) => keep,
        ExportDraft::Raster(RasterExport::Pdf)
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

fn pages(span: PageSpan, ontune: EventHandler<ExportOption>) -> Element {
    rsx! {
        FieldRow { label: "Pages", layout: RowLayout::Form,
            {segments(
                "Pages",
                &[PageSpan::All, PageSpan::Current, PageSpan::Between],
                |span| match span {
                    PageSpan::All => "All".to_owned(),
                    PageSpan::Current => "Current page".to_owned(),
                    PageSpan::Between => "From – to".to_owned(),
                },
                Some(span),
                EventHandler::new(move |to| ontune.call(ExportOption::Pages(to))),
            )}
        }
    }
}

fn page_range(
    draft: ExportDraft,
    facts: ExportFacts,
    ontune: EventHandler<ExportOption>,
) -> Element {
    let (first, last) = match pages_of(draft) {
        Some(PageSelection::Range(range)) => (range.first(), range.last()),
        Some(PageSelection::All) | None => (PageIndex(0), PageIndex(0)),
    };
    let count = facts
        .pages
        .map_or(1, |count| i32::try_from(count.get()).unwrap_or(i32::MAX));
    rsx! {
        FieldRow { label: "From", layout: RowLayout::Form,
            {stepper(
                "First page",
                number(first),
                1,
                count,
                EventHandler::new(move |to: i32| ontune.call(ExportOption::PagesFrom(u32::try_from(to).unwrap_or(1)))),
            )}
        }
        FieldRow { label: "To", layout: RowLayout::Form,
            {stepper(
                "Last page",
                number(last),
                1,
                count,
                EventHandler::new(move |to: i32| ontune.call(ExportOption::PagesTo(u32::try_from(to).unwrap_or(1)))),
            )}
        }
    }
}

fn image_format(draft: ExportDraft, ontune: EventHandler<ExportOption>) -> Element {
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

fn resolution(draft: ExportDraft, ontune: EventHandler<ExportOption>) -> Element {
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

fn paper(draft: ExportDraft, ontune: EventHandler<ExportOption>) -> Element {
    let current = match draft {
        ExportDraft::Text(TextExport::Pdf(layout)) => layout.paper,
        ExportDraft::Raster(_)
        | ExportDraft::Pdf(_)
        | ExportDraft::Text(TextExport::PlainText)
        | ExportDraft::Media(_) => PaperSize::default(),
    };
    rsx! {
        FieldRow { label: "Paper", layout: RowLayout::Form,
            {segments(
                "Paper",
                PaperSize::ALL,
                |paper| paper.label().to_owned(),
                Some(current),
                EventHandler::new(move |paper| ontune.call(ExportOption::Paper(paper))),
            )}
        }
    }
}

fn orientation(draft: ExportDraft, ontune: EventHandler<ExportOption>) -> Element {
    let current = match draft {
        ExportDraft::Text(TextExport::Pdf(layout)) => layout.orientation,
        ExportDraft::Raster(_)
        | ExportDraft::Pdf(_)
        | ExportDraft::Text(TextExport::PlainText)
        | ExportDraft::Media(_) => Orientation::default(),
    };
    rsx! {
        FieldRow { label: "Orientation", layout: RowLayout::Form,
            {segments(
                "Orientation",
                Orientation::ALL,
                |way| way.label().to_owned(),
                Some(current),
                EventHandler::new(move |way| ontune.call(ExportOption::Orientation(way))),
            )}
        }
    }
}

fn range(draft: ExportDraft, facts: ExportFacts, ontune: EventHandler<ExportOption>) -> Element {
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

fn bitrate(draft: ExportDraft, ontune: EventHandler<ExportOption>) -> Element {
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
