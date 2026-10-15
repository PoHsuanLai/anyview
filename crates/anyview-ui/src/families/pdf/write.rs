//! The words of a note or a text box, typed where the mark is.

use crate::MarkupIn;
use dioxus::prelude::*;
use ds::components::fields::text_field_model::FieldRows;
use ds::prelude::{FieldBezel, FieldFocus, FieldKind, TextField};

/// The mark being written in: which one, where its box is, and what it says.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Writing {
    /// What the box is called to the person ("Note", "Text box").
    pub label: &'static str,
    /// The box on the page: inline CSS, percentages of the page.
    pub style: String,
    pub text: String,
}

/// A box of text over the page, with the caret in it. Letting go of the keyboard, or Escape, ends
/// the writing.
#[component]
pub(super) fn WriteBox(writing: Writing, mark: EventHandler<MarkupIn>) -> Element {
    let Writing { label, style, text } = writing;
    rsx! {
        div {
            class: "viewer-write",
            style,
            onpointerdown: move |event: PointerEvent| event.stop_propagation(),
            TextField {
                label,
                value: text,
                kind: FieldKind::Multiline,
                rows: FieldRows::Two,
                bezel: FieldBezel::Plain,
                focus: FieldFocus::OnMount,
                oninput: move |text: String| mark.call(MarkupIn::Words(text)),
                onblur: move |()| mark.call(MarkupIn::Done),
                onkey: move |event: KeyboardEvent| {
                    if event.key() == Key::Escape {
                        event.prevent_default();
                        event.stop_propagation();
                        mark.call(MarkupIn::Done);
                    } else {
                        event.stop_propagation();
                    }
                },
            }
        }
    }
}
