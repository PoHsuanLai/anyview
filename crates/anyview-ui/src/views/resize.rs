//! Adjust Size: a dialog that appears in place, as the export dialog does, with a width and a
//! height in pixels or as a percent of the picture now, kept in proportion unless the person
//! unlocks it, and the size the picture will have.

use super::sheet::confirms;
use crate::{ResizeChange, ResizeDraft, ResizeProportion, ResizeUnit};
use dioxus::prelude::*;
use ds::components::controls::button_model::Answers;
use ds::components::controls::checkbox::Checkbox;
use ds::components::controls::segmented::Tracking;
use ds::components::fields::field_row::{FieldRow, RowLayout};
use ds::components::fields::stepper::model::StepRange;
use ds::components::fields::stepper::view::Stepper;
use ds::components::overlays::sheet_attach::Attach;
use ds::prelude::{Button, Choice, Form, FormSection, SegmentedControl, Sheet};
use ds_core::vocab::Check;

/// A side's stepper over `1..=most`, in the unit the draft counts.
fn side(
    label: &'static str,
    value: u32,
    most: u32,
    suffix: &'static str,
    onchange: EventHandler<u32>,
) -> Element {
    let value = i32::try_from(value).unwrap_or(i32::MAX);
    let most = i32::try_from(most).unwrap_or(i32::MAX);
    rsx! {
        FieldRow { label, layout: RowLayout::Form,
            div { class: "viewer-export-slider",
                Stepper {
                    label,
                    value,
                    range: StepRange::new(1, most, 1),
                    onchange: move |to: i32| onchange.call(u32::try_from(to).unwrap_or(1)),
                }
                span { class: "viewer-export-readout", "{suffix}" }
            }
        }
    }
}

/// Choosing the picture's new size.
#[component]
pub(super) fn ResizeSheet(
    draft: ResizeDraft,
    onchange: EventHandler<ResizeChange>,
    onconfirm: EventHandler<()>,
    oncancel: EventHandler<()>,
) -> Element {
    let suffix = match draft.unit() {
        ResizeUnit::Pixels => "pixels",
        ResizeUnit::Percent => "percent",
    };
    let units = [ResizeUnit::Pixels, ResizeUnit::Percent]
        .iter()
        .map(|unit| Choice::new(*unit, unit.label()))
        .collect::<Vec<_>>();
    let kept = match draft.proportion() {
        ResizeProportion::Kept => Check::On,
        ResizeProportion::Free => Check::Off,
    };
    let result = draft.size();
    rsx! {
        Sheet {
            label: "Adjust Size",
            attach: Attach::Centre,
            onclose: move |()| oncancel.call(()),
            div { class: "viewer-sheet viewer-resize", onkeydown: move |event| confirms(&event, onconfirm),
                Form {
                    FormSection { title: "Adjust Size".to_owned(),
                        FieldRow { label: "Count in", layout: RowLayout::Form,
                            SegmentedControl::<ResizeUnit> {
                                label: "Count in",
                                choices: units,
                                tracking: Tracking::SelectOne(draft.unit()),
                                onchange: move |unit: ResizeUnit| onchange.call(ResizeChange::Unit(unit)),
                            }
                        }
                        {side(
                            "Width",
                            draft.width_shown(),
                            draft.most(),
                            suffix,
                            EventHandler::new(move |to: u32| onchange.call(ResizeChange::Width(to))),
                        )}
                        {side(
                            "Height",
                            draft.height_shown(),
                            draft.most(),
                            suffix,
                            EventHandler::new(move |to: u32| onchange.call(ResizeChange::Height(to))),
                        )}
                        FieldRow { label: "Proportions", layout: RowLayout::Form,
                            Checkbox {
                                label: "Scale proportionally",
                                value: kept,
                                onchange: move |to: Check| {
                                    onchange.call(ResizeChange::Proportion(match to {
                                        Check::On | Check::Mixed => ResizeProportion::Kept,
                                        Check::Off => ResizeProportion::Free,
                                    }));
                                },
                            }
                        }
                    }
                }
                p { class: "viewer-sheet-note", "The picture will be {result.width.0} \u{d7} {result.height.0} pixels." }
                div { class: "viewer-sheet-buttons",
                    Button { label: "Cancel", answers: Answers::Escape, onclick: move |_| oncancel.call(()) }
                    Button { label: "OK", answers: Answers::Return, onclick: move |_| onconfirm.call(()) }
                }
            }
        }
    }
}
