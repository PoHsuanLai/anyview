//! The find bar: a search field, how the search stands, and the steps through its hits. It
//! sends what the person did to the stage machine (`TextIn`) and draws what the machine says;
//! the keys typed in the field go to the window (`StageCx::typing`), which knows the chords.

use crate::families::view::StageCx;
use crate::{FindHits, StageIn, TextIn, TypedText};
use dioxus::prelude::*;
use ds::components::controls::button::Button;
use ds::components::controls::button_model::{Bezel, ImagePosition};
use ds::prelude::{FieldFocus, FieldKind, Icon, IconSource, TextField};
use ds::style::tokens::control_size::ControlSize;

/// How the search stands, in words: nothing before something is typed.
fn standing(hits: FindHits) -> String {
    match hits {
        FindHits::Idle => String::new(),
        FindHits::Pending => "Searching…".to_owned(),
        FindHits::NoMatch => "No matches".to_owned(),
        FindHits::Found(_) => match (hits.current(), hits.count()) {
            (Some(current), Some(count)) => format!("{} of {}", current.0 + 1, count.0),
            (None, _) | (_, None) => String::new(),
        },
    }
}

/// The bar over the top of the text, for the find `query` whose search stands at `hits`.
#[component]
pub(super) fn FindBar(query: TypedText, hits: FindHits, cx: StageCx) -> Element {
    let send = cx.send;
    let typing = cx.typing;
    let standing = standing(hits);
    rsx! {
        div { class: "viewer-find", role: "search",
            TextField {
                label: "Find",
                placeholder: "Find",
                kind: FieldKind::Search,
                size: ControlSize::Small,
                value: query.as_str().to_owned(),
                focus: FieldFocus::OnMount,
                oninput: move |text: String| send.call(StageIn::Text(TextIn::Find(TypedText::new(text)))),
                onkey: move |event: KeyboardEvent| typing.call(event),
            }
            span { class: "viewer-find-standing", "data-standing": hits_word(hits), "{standing}" }
            Button {
                label: "Previous match",
                bezel: Bezel::Toolbar,
                size: ControlSize::Small,
                image: ImagePosition::Only,
                icon: Some(IconSource::from(Icon::ChevronUp)),
                onclick: move |_| send.call(StageIn::Text(TextIn::PreviousHit)),
            }
            Button {
                label: "Next match",
                bezel: Bezel::Toolbar,
                size: ControlSize::Small,
                image: ImagePosition::Only,
                icon: Some(IconSource::from(Icon::ChevronDown)),
                onclick: move |_| send.call(StageIn::Text(TextIn::NextHit)),
            }
            Button {
                label: "Close find",
                bezel: Bezel::Toolbar,
                size: ControlSize::Small,
                image: ImagePosition::Only,
                icon: Some(IconSource::from(Icon::X)),
                onclick: move |_| send.call(StageIn::Text(TextIn::CloseFind)),
            }
        }
    }
}

/// The state of the search as a word, for the stylesheet to colour "No matches".
fn hits_word(hits: FindHits) -> &'static str {
    match hits {
        FindHits::Idle => "idle",
        FindHits::Pending => "pending",
        FindHits::NoMatch => "none",
        FindHits::Found(_) => "found",
    }
}
