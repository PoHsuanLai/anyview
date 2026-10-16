//! The row an export option is drawn as.

use super::media::{bitrate, range};
use super::pages::{orientation, page_range, pages, paper};
use super::picture::{image_format, metadata, quality, resolution, size};
use crate::{ExportControl, ExportDraft, ExportFacts, ExportOption, PageSpan};
use dioxus::prelude::*;

/// One option of the export, as a row of the form.
pub(in crate::views) fn option_row(
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
