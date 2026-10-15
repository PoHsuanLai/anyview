//! A form field placed over its page. While the document draws it, it is a transparent box that
//! waits to be pressed, so a form that is only looked at looks as the file drew it. Once the
//! keyboard is in it, or the person has changed it, it is the live control: a text field, a
//! check box, a radio button, a drop-down or a list.

use crate::MarkupIn;
use crate::views::press::CHORD_KEYS;
use anyview_core::{FormWidget, WidgetKind};
use dioxus::html::ModifiersInteraction as _;
use dioxus::prelude::*;
use ds::components::fields::text_field_model::FieldRows;
use ds::components::menus::pop_up_button::PopUpButton;
use ds::focus::soon::focus_soon;
use ds::prelude::{FieldBezel, FieldFocus, FieldKind, MenuItem, Shown, TextField};
use ds_core::word::Word;

/// How a field is, to the person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub(super) enum FieldState {
    /// The document draws it; nothing has happened to it.
    Idle,
    /// The keyboard is in it.
    Held,
    /// The person changed it.
    Filled,
}

/// A field to draw: which widget of the form, where it is on its page, and what it holds.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct FieldView {
    pub index: usize,
    pub widget: FormWidget,
    /// The box on the page: inline CSS, percentages of the page.
    pub style: String,
    pub value: String,
    pub on: bool,
    pub state: FieldState,
}

/// Whether a key is one the field keeps for itself: anything but a chord for the window.
fn typed(event: &KeyboardEvent) -> bool {
    !event.modifiers().intersects(CHORD_KEYS)
}

/// The keys of a field the keyboard is in: Tab and Shift Tab go to the next and the one before,
/// Escape lets go, and the rest is typing, which the window does not hear.
fn keyed(event: &KeyboardEvent, index: usize, mark: EventHandler<MarkupIn>) {
    let key = event.key();
    if key == Key::Tab {
        event.prevent_default();
        event.stop_propagation();
        mark.call(if event.modifiers().contains(Modifiers::SHIFT) {
            MarkupIn::Previous
        } else {
            MarkupIn::Next
        });
    } else if key == Key::Escape {
        event.prevent_default();
        event.stop_propagation();
        mark.call(MarkupIn::Blur(index));
    } else if typed(event) {
        event.stop_propagation();
    }
}

/// One field, drawn at its place.
#[component]
pub(super) fn FormField(view: FieldView, mark: EventHandler<MarkupIn>) -> Element {
    let FieldView {
        index,
        widget,
        style,
        value,
        on,
        state,
    } = view;
    let kind = widget.kind;
    let toggles = kind.is_toggle();
    if state == FieldState::Idle {
        return rsx! {
            div {
                class: "viewer-field",
                role: "group",
                "aria-label": "{widget.name}",
                "data-kind": kind.slug(),
                "data-state": state.slug(),
                style,
                onpointerdown: move |event: PointerEvent| event.stop_propagation(),
                onclick: move |_| {
                    if toggles {
                        mark.call(MarkupIn::Toggle(index));
                    } else {
                        mark.call(MarkupIn::Focus(index));
                    }
                },
            }
        };
    }
    let held = state == FieldState::Held;
    let control = match kind {
        WidgetKind::Text | WidgetKind::TextArea | WidgetKind::Secret => {
            let (field_kind, rows) = match kind {
                WidgetKind::TextArea => (FieldKind::Multiline, FieldRows::Four),
                WidgetKind::Secret => (FieldKind::Secure, FieldRows::default()),
                WidgetKind::Text
                | WidgetKind::Check
                | WidgetKind::Radio
                | WidgetKind::Choice
                | WidgetKind::List => (FieldKind::Plain, FieldRows::default()),
            };
            rsx! {
                TextField {
                    label: widget.name.clone(),
                    value,
                    kind: field_kind,
                    rows,
                    bezel: FieldBezel::Plain,
                    focus: if held { FieldFocus::OnMount } else { FieldFocus::Manual },
                    oninput: move |text: String| mark.call(MarkupIn::Type { widget: index, text }),
                    onfocus: move |()| mark.call(MarkupIn::Focus(index)),
                    onblur: move |()| mark.call(MarkupIn::Blur(index)),
                    onkey: move |event: KeyboardEvent| keyed(&event, index, mark),
                }
            }
        }
        WidgetKind::Check | WidgetKind::Radio => rsx! {
            button {
                r#type: "button",
                class: "viewer-field-toggle",
                role: if kind == WidgetKind::Radio { "radio" } else { "checkbox" },
                "aria-checked": if on { "true" } else { "false" },
                "aria-label": "{widget.name}",
                "data-kind": kind.slug(),
                "data-on": if on { "on" } else { "off" },
                onmounted: move |event: MountedEvent| {
                    if held {
                        focus_soon(event.data());
                    }
                },
                onfocus: move |_| mark.call(MarkupIn::Focus(index)),
                onblur: move |_| mark.call(MarkupIn::Blur(index)),
                onkeydown: move |event: KeyboardEvent| keyed(&event, index, mark),
                onclick: move |_| mark.call(MarkupIn::Toggle(index)),
                span { class: "viewer-field-mark" }
            }
        },
        WidgetKind::Choice => {
            let items: Vec<MenuItem<String>> = widget
                .options
                .iter()
                .enumerate()
                .map(|(at, option)| {
                    MenuItem::new(
                        widget.exports.get(at).unwrap_or(option).clone(),
                        option.clone(),
                    )
                })
                .collect();
            rsx! {
                PopUpButton::<String> {
                    items,
                    value: Some(value),
                    start: if held { Shown::Visible } else { Shown::Hidden },
                    onpick: move |text: String| mark.call(MarkupIn::Type { widget: index, text }),
                    on_open_change: move |shown: Shown| {
                        if shown == Shown::Hidden {
                            mark.call(MarkupIn::Blur(index));
                        }
                    },
                }
            }
        }
        WidgetKind::List => rsx! {
            div { class: "viewer-field-list", role: "listbox", "aria-label": "{widget.name}",
                for (at, option) in widget.options.iter().cloned().enumerate() {
                    {
                        let chosen = widget.exports.get(at).unwrap_or(&option).clone();
                        let on = chosen == value;
                        rsx! {
                            button {
                                r#type: "button",
                                class: "viewer-field-option",
                                role: "option",
                                "aria-selected": if on { "true" } else { "false" },
                                "data-on": if on { "on" } else { "off" },
                                onclick: move |_| {
                                    mark.call(MarkupIn::Type { widget: index, text: chosen.clone() });
                                },
                                "{option}"
                            }
                        }
                    }
                }
            }
        },
    };
    rsx! {
        div {
            class: "viewer-field",
            "data-kind": kind.slug(),
            "data-state": state.slug(),
            style,
            onpointerdown: move |event: PointerEvent| event.stop_propagation(),
            {control}
        }
    }
}
