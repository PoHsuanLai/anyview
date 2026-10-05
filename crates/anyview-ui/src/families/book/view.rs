//! A section on screen: the page a worker unpacked, in a sealed frame. The view asks for the
//! section the stage machine is at; every move is the machine's.

use super::doc::BookDoc;
use super::frame;
use crate::families::view::{Held, StageCx};
use crate::{BookStage, Stage};
use anyview_core::SectionIndex;
use dioxus::prelude::*;

/// The section the stage is at, when it is the book stage.
pub(super) fn section_of(stage: &Stage) -> Option<SectionIndex> {
    match stage {
        Stage::Book(BookStage::Reading { section }) => Some(*section),
        Stage::NoStage | Stage::Raster(_) | Stage::Pdf(_) | Stage::Media(_) | Stage::Text(_) => {
            None
        }
    }
}

#[component]
pub(super) fn BookContent(doc: Held<BookDoc>, cx: StageCx) -> Element {
    let wanted = section_of(&cx.stage);
    let held = cx.section.as_ref().map(|page| page.0.section);
    let ask = cx.ask_section;
    use_effect(use_reactive!(|wanted, held| {
        if let Some(wanted) = wanted
            && held != Some(wanted)
        {
            ask.call(wanted);
        }
    }));
    let _ = &doc;
    match &cx.section {
        Some(page) => {
            let document = frame::document(&page.0, &cx.frame);
            rsx! {
                div { class: "viewer-book-room",
                    iframe { class: "viewer-frame", "data-frame-tag": "book", srcdoc: document }
                }
            }
        }
        None => rsx! { div { class: "viewer-book-room" } },
    }
}
