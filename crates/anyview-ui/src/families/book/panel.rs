//! The side panel's contents list for an EPUB: each line leads to its chapter by handing the stage
//! machine a section.

use super::doc::BookDoc;
use crate::families::view::{Held, StageCx};
use crate::{BookIn, StageIn};
use dioxus::prelude::*;
use ds::prelude::Row;
use ds_core::vocab::{Availability, RowState};

#[component]
pub(super) fn Contents(doc: Held<BookDoc>, cx: StageCx) -> Element {
    let send = cx.send;
    rsx! {
        div { class: "viewer-outline", role: "tree",
            for (at, entry) in doc.0.contents().iter().enumerate() {
                {
                    let section = entry.section;
                    let availability = match section {
                        Some(_) => Availability::Enabled,
                        None => Availability::Disabled,
                    };
                    rsx! {
                        div {
                            key: "{at}",
                            class: "viewer-outline-entry",
                            style: "margin-left:calc(var(--s-12) * {entry.depth})",
                            Row {
                                title: entry.title.clone(),
                                state: RowState { availability, ..RowState::default() },
                                onclick: move |_| {
                                    if let Some(section) = section {
                                        send.call(StageIn::Book(BookIn::GoTo(section)));
                                    }
                                },
                            }
                        }
                    }
                }
            }
        }
    }
}
