//! The side panel's two tabs for a PDF: the page thumbnails and the outline. A thumbnail is a page
//! a worker drew small and uploaded, asked for only for the pages near the reader; both lists go
//! to a page by handing the stage machine a place.

use super::doc::PdfDoc;
use super::live::page_view;
use super::work::{PdfAsk, PdfTask};
use crate::families::view::{Held, StageCx};
use crate::io::Job;
use crate::{Destination, PdfIn, StageIn};
use anyview_core::{PageIndex, Permille};
use dioxus::prelude::*;
use ds::prelude::{Row, Selection};
use ds_blitz::{TextureFit, TextureLayer, use_gpu};
use ds_core::vocab::{Availability, RowState};
use std::sync::Arc;

/// The width of a thumbnail, in logical pixels.
const WIDTH: f32 = 112.0;

/// How many pages either side of the reader get a thumbnail drawn.
const NEAR: u32 = 12;

/// The pages a thumbnail is drawn for when the reader is on `reader`.
fn near(reader: PageIndex, pages: u32) -> impl Iterator<Item = PageIndex> {
    let first = reader.0.saturating_sub(NEAR);
    let last = reader.0.saturating_add(NEAR).min(pages.saturating_sub(1));
    (first..=last).map(PageIndex)
}

fn go_to(page: PageIndex) -> StageIn {
    StageIn::Pdf(PdfIn::GoTo(Destination {
        page,
        offset: Permille(0),
    }))
}

#[component]
pub(super) fn Thumbnails(doc: Held<PdfDoc>, cx: StageCx) -> Element {
    let gpu = use_gpu();
    let reader = page_view(&cx.stage).map_or(PageIndex(0), |view| view.page);
    let ready = gpu.device().is_some();
    let (live, work, ticket) = (cx.pdf, cx.work, cx.ticket);
    let asking = Arc::clone(&doc.0);
    use_effect(use_reactive!(|reader, ready| {
        if !ready {
            return;
        }
        for page in near(reader, asking.pages().get()) {
            if live.with_mut(|live| live.thumb_wanted(page)) {
                let ask = PdfAsk::Thumb { page };
                let task = PdfTask::new(ticket, Arc::clone(&asking), gpu.clone(), ask);
                work.call(Job::Pdf(task));
            }
        }
    }));
    let send = cx.send;
    let live = live.read();
    rsx! {
        div { class: "viewer-thumbs",
            for at in 0..doc.0.pages().get() {
                {
                    let page = PageIndex(at);
                    let size = doc.0.size_of(page);
                    let height = WIDTH * size.height.0 as f32 / size.width.0 as f32;
                    let selection = if page == reader { Selection::Selected } else { Selection::Unselected };
                    let texture = live.thumb(page).cloned();
                    rsx! {
                        Row {
                            key: "{at}",
                            title: format!("Page {}", at + 1),
                            state: RowState { selection, ..RowState::default() },
                            onclick: move |_| send.call(go_to(page)),
                            content: rsx! {
                                span { class: "viewer-thumb",
                                    span {
                                        class: "viewer-thumb-page",
                                        style: "width:{WIDTH}px;height:{height}px",
                                        TextureLayer { texture, fit: TextureFit::Fill }
                                        // A texture layer swallows a click, so the row around
                                        // it would never hear one; this cover is what the
                                        // pointer lands on.
                                        span { class: "viewer-thumb-cover" }
                                    }
                                    span { class: "viewer-thumb-number", "{at + 1}" }
                                }
                            },
                        }
                    }
                }
            }
        }
    }
}

#[component]
pub(super) fn Outline(doc: Held<PdfDoc>, cx: StageCx) -> Element {
    let send = cx.send;
    rsx! {
        div { class: "viewer-outline", role: "tree",
            for (at, entry) in doc.0.outline.iter().enumerate() {
                {
                    let depth = entry.depth;
                    let page = entry.page;
                    let availability = match page {
                        Some(_) => Availability::Enabled,
                        None => Availability::Disabled,
                    };
                    rsx! {
                        div {
                            key: "{at}",
                            class: "viewer-outline-entry",
                            style: "margin-left:calc(var(--s-12) * {depth})",
                            Row {
                                title: entry.title.clone(),
                                state: RowState { availability, ..RowState::default() },
                                onclick: move |_| {
                                    if let Some(page) = page {
                                        send.call(go_to(page));
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
