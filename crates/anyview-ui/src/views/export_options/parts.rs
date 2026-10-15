//! The controls the option rows are built from: a segmented choice and a stepper.

use crate::ExportDraft;
use anyview_core::{PageIndex, PageSelection, PdfExport};
use dioxus::prelude::*;
use ds::components::controls::segmented::Tracking;
use ds::components::fields::stepper::model::StepRange;
use ds::components::fields::stepper::view::Stepper;
use ds::prelude::{Choice, SegmentedControl};

/// A segmented control that picks one of `values`, each said by `say`. With no `current` (a value
/// the control has no segment for) none is lit.
pub(super) fn segments<T: Clone + PartialEq + 'static>(
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
pub(super) fn stepper(
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
pub(super) fn number(page: PageIndex) -> i32 {
    i32::try_from(page.0.saturating_add(1)).unwrap_or(i32::MAX)
}
/// The pages the draft's page choice covers.
pub(super) fn pages_of(draft: ExportDraft) -> Option<PageSelection> {
    match draft {
        ExportDraft::Pdf(PdfExport::Pdf(pages) | PdfExport::PageImages(pages, ..)) => Some(pages),
        ExportDraft::Raster(_)
        | ExportDraft::Pdf(PdfExport::PlainText | PdfExport::Markdown)
        | ExportDraft::Text(_)
        | ExportDraft::Media(_) => None,
    }
}
