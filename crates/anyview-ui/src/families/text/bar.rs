//! The find bar over the text: the shared bar, wired to the text machine's inputs.

use crate::families::find_bar::{FindBar, FindStep};
use crate::families::view::StageCx;
use crate::{FindHits, StageIn, TextIn, TypedText};
use dioxus::prelude::*;

/// The bar over the top of the text, for the find `query` whose search stands at `hits`.
#[component]
pub(super) fn TextFinding(query: TypedText, hits: FindHits, cx: StageCx) -> Element {
    let send = cx.send;
    rsx! {
        FindBar {
            query,
            hits,
            typing: cx.typing,
            onfind: move |text: TypedText| send.call(StageIn::Text(TextIn::Find(text))),
            onstep: move |step: FindStep| {
                send.call(StageIn::Text(match step {
                    FindStep::Next => TextIn::NextHit,
                    FindStep::Previous => TextIn::PreviousHit,
                    FindStep::Close => TextIn::CloseFind,
                }));
            },
        }
    }
}
