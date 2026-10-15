//! A value typed into where it is read: the cell of a table or the value or key of a node. It is
//! quire's plain text field (no box, the font of the row), started with the text it is given and
//! focused as it appears. Return takes the text, Tab takes it and goes on when the family has a next
//! place, Esc lets it go, and the caret leaving takes it where it is. The keys that mean something
//! to the field stay with it: the window does not hear them.

use crate::views::press::{CHORD_KEYS, COMMAND_KEYS};
use crate::{AfterTyping, TypedText, TypingIn};
use dioxus::html::ModifiersInteraction as _;
use dioxus::prelude::*;
use ds::prelude::{FieldBezel, FieldFocus, TextField};

/// Where the typing stands: still going, or ended by a key (so the caret leaving afterwards adds
/// nothing).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Typing {
    Going,
    Ended,
}

/// The text field for a value that starts as `seed`. `on_return` and `on_tab` say where the cursor
/// goes after the key; no `on_tab` leaves Tab to the window.
#[component]
pub(crate) fn InlineEdit(
    seed: String,
    on_return: AfterTyping,
    on_tab: Option<AfterTyping>,
    send: EventHandler<TypingIn>,
) -> Element {
    let mut draft = use_signal(|| seed.clone());
    let mut standing = use_signal(|| Typing::Going);
    let mut end = move |input: TypingIn| {
        standing.set(Typing::Ended);
        send.call(input);
    };
    let take =
        move |then: AfterTyping| TypingIn::Commit(TypedText::new(draft.peek().clone()), then);
    let start = seed.clone();
    rsx! {
        span { class: "viewer-inline-edit",
            TextField {
                label: "Edit",
                value: draft(),
                bezel: FieldBezel::Plain,
                focus: FieldFocus::OnMount,
                oninput: move |text: String| draft.set(text),
                onkey: move |event: KeyboardEvent| {
                    let held = event.modifiers();
                    let plain = !held.intersects(CHORD_KEYS);
                    let key = event.key();
                    // Numbers' first Undo while a cell is typed in takes back the typing, not the
                    // edit before it.
                    let undo = held.intersects(COMMAND_KEYS)
                        && !held.intersects(Modifiers::ALT | Modifiers::SHIFT)
                        && matches!(&key, Key::Character(c) if c.eq_ignore_ascii_case("z"));
                    if undo && *draft.peek() != start {
                        event.prevent_default();
                        event.stop_propagation();
                        draft.set(start.clone());
                    } else if plain && key == Key::Enter {
                        event.prevent_default();
                        event.stop_propagation();
                        end(take(on_return));
                    } else if plain && key == Key::Tab && on_tab.is_some() {
                        if let Some(then) = on_tab {
                            event.prevent_default();
                            event.stop_propagation();
                            end(take(then));
                        }
                    } else if key == Key::Escape {
                        event.prevent_default();
                        event.stop_propagation();
                        end(TypingIn::Cancel);
                    } else if plain {
                        // A chord (a Save, an Undo) is the window's; what is typed is the field's.
                        event.stop_propagation();
                    }
                },
                onblur: move |()| {
                    if *standing.peek() == Typing::Going {
                        end(take(AfterTyping::Stay));
                    }
                },
            }
        }
    }
}
