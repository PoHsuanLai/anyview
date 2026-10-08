//! The find bar of every stage that searches: a search field, how the search stands, and the
//! steps through its hits. It draws what the stage machine says and tells the stage what the
//! person did; the keys typed in the field go to the window (`StageCx::typing`), which knows the
//! chords. A stage wraps it with the inputs of its own machine, so the bar is one thing.

use crate::{FindHits, TypedText};
use dioxus::prelude::*;
use ds::components::controls::button::Button;
use ds::components::controls::button_model::{Bezel, ImagePosition};
use ds::prelude::{FieldFocus, FieldKind, Icon, IconSource, TextField};
use ds::style::tokens::control_size::ControlSize;

/// What a press on the bar's buttons asks of the stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FindStep {
    /// The hit after the current one.
    Next,
    /// The hit before the current one.
    Previous,
    /// Put the find away.
    Close,
}

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

/// The state of the search as a word, for the stylesheet to colour "No matches".
fn hits_word(hits: FindHits) -> &'static str {
    match hits {
        FindHits::Idle => "idle",
        FindHits::Pending => "pending",
        FindHits::NoMatch => "none",
        FindHits::Found(_) => "found",
    }
}

/// The bar over the top of the content, for the find `query` whose search stands at `hits`.
#[component]
pub(super) fn FindBar(
    query: TypedText,
    hits: FindHits,
    typing: EventHandler<KeyboardEvent>,
    onfind: EventHandler<TypedText>,
    onstep: EventHandler<FindStep>,
) -> Element {
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
                oninput: move |text: String| onfind.call(TypedText::new(text)),
                onkey: move |event: KeyboardEvent| typing.call(event),
            }
            span { class: "viewer-find-standing", "data-standing": hits_word(hits), "{standing}" }
            Button {
                label: "Previous match",
                title: Some("Previous match".to_owned()),
                bezel: Bezel::Toolbar,
                size: ControlSize::Small,
                image: ImagePosition::Only,
                icon: Some(IconSource::from(Icon::ChevronUp)),
                onclick: move |_| onstep.call(FindStep::Previous),
            }
            Button {
                label: "Next match",
                title: Some("Next match".to_owned()),
                bezel: Bezel::Toolbar,
                size: ControlSize::Small,
                image: ImagePosition::Only,
                icon: Some(IconSource::from(Icon::ChevronDown)),
                onclick: move |_| onstep.call(FindStep::Next),
            }
            Button {
                label: "Close find",
                title: Some("Close find".to_owned()),
                bezel: Bezel::Toolbar,
                size: ControlSize::Small,
                image: ImagePosition::Only,
                icon: Some(IconSource::from(Icon::X)),
                onclick: move |_| onstep.call(FindStep::Close),
            }
        }
    }
}
