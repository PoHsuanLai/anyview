//! The find bar: a field, where the search stands, and the buttons for the next and the previous
//! hit. It shows while the stage machine is finding and holds nothing of its own: what is typed
//! is the machine's query, and the buttons are machine inputs.

use crate::families::view::StageCx;
use crate::{FindHits, PdfIn, PdfStage, Stage, StageIn, TypedText};
use dioxus::prelude::*;
use ds::components::controls::button_model::{Bezel, ImagePosition};
use ds::prelude::{Button, FieldFocus, FieldKind, Icon, Material, Surface, TextField};

/// Where the search stands, in words.
fn status(hits: FindHits) -> String {
    match hits {
        FindHits::Idle => String::new(),
        FindHits::Pending => "Searching…".to_owned(),
        FindHits::NoMatch => "No matches".to_owned(),
        FindHits::Found(_) => match hits.current() {
            Some(current) => format!("{} of {}", current.0 + 1, hits.count().0),
            None => String::new(),
        },
    }
}

/// What a key typed in the field means to the bar. Enter steps, Escape and chords are the
/// window's (they reach it), and every other key is the field's alone: a letter typed in the
/// field is not a command of the stage.
fn keyed(event: &KeyboardEvent, send: EventHandler<StageIn>) {
    let chord = event
        .modifiers()
        .intersects(Modifiers::CONTROL | Modifiers::META | Modifiers::ALT);
    let key = event.key();
    if key == Key::Enter {
        event.stop_propagation();
        let step = if event.modifiers().contains(Modifiers::SHIFT) {
            PdfIn::PreviousHit
        } else {
            PdfIn::NextHit
        };
        send.call(StageIn::Pdf(step));
    } else if key != Key::Escape && !chord {
        event.stop_propagation();
    }
}

#[component]
pub(super) fn FindBar(cx: StageCx) -> Element {
    let Stage::Pdf(PdfStage::Finding { query, hits, .. }) = &cx.stage else {
        return rsx! {};
    };
    let (query, hits) = (query.clone(), *hits);
    let send = cx.send;
    rsx! {
        div { class: "viewer-find", role: "search",
          Surface { material: Material::Bar,
            div { class: "viewer-find-row",
            TextField {
                label: "Find in document",
                kind: FieldKind::Search,
                value: query.as_str().to_owned(),
                placeholder: "Find",
                focus: FieldFocus::OnMount,
                oninput: move |text: String| send.call(StageIn::Pdf(PdfIn::Find(TypedText::new(text)))),
                onkey: move |event: KeyboardEvent| keyed(&event, send),
            }
            span { class: "viewer-find-status", "{status(hits)}" }
            Button {
                label: "Previous match",
                icon: Some(Icon::ChevronUp),
                image: ImagePosition::Only,
                bezel: Bezel::Toolbar,
                onclick: move |_| send.call(StageIn::Pdf(PdfIn::PreviousHit)),
            }
            Button {
                label: "Next match",
                icon: Some(Icon::ChevronDown),
                image: ImagePosition::Only,
                bezel: Bezel::Toolbar,
                onclick: move |_| send.call(StageIn::Pdf(PdfIn::NextHit)),
            }
            Button {
                label: "Close find",
                icon: Some(Icon::X),
                image: ImagePosition::Only,
                bezel: Bezel::Toolbar,
                onclick: move |_| send.call(StageIn::Pdf(PdfIn::CloseFind)),
            }
            }
          }
        }
    }
}
