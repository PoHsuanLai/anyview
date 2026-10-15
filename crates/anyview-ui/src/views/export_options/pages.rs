//! The options that choose pages and paper.

use super::parts::{number, pages_of, segments, stepper};
use crate::{ExportDraft, ExportFacts, ExportOption, PageSpan};
use anyview_core::{Orientation, PageIndex, PageSelection, PaperSize, TextExport};
use dioxus::prelude::*;
use ds::components::fields::field_row::{FieldRow, RowLayout};

pub(super) fn pages(span: PageSpan, ontune: EventHandler<ExportOption>) -> Element {
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
pub(super) fn page_range(
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
pub(super) fn paper(draft: ExportDraft, ontune: EventHandler<ExportOption>) -> Element {
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
pub(super) fn orientation(draft: ExportDraft, ontune: EventHandler<ExportOption>) -> Element {
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
