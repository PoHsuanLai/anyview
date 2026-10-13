//! The side panel's two tabs for a PDF: the page thumbnails and the outline. Both are quire
//! `VirtualList`s, so only the rows near the viewport exist: a thumbnail is asked of a worker when
//! its row is mounted, and a large outline costs what is in view. Both lists go to a page by
//! handing the stage machine a place. The outline folds: which bookmarks are folded and where the
//! cursor is belong to `OutlineFold`, which the shelf holds.

use super::doc::PdfDoc;
use super::live::page_view;
use super::page::Emphasis;
use super::shelf::PdfShelf;
use super::work::{PdfAsk, PdfTask};
use crate::families::rows::compact_px;
use crate::families::view::{Held, StageCx};
use crate::io::Job;
use crate::{Destination, PdfIn, StageIn, Ticket};
use anyview_core::{PageIndex, Permille};
use anyview_machines::{OutlineIn, RowStep};
use dioxus::prelude::*;
use ds::components::lists::row::row::{Outline as RowOutline, Row};
use ds::components::lists::row::size::RowSize;
use ds::components::lists::virtual_list::{RowHeight, VirtualList};
use ds::prelude::Px;
use ds_blitz::{TextureFit, TextureLayer, use_gpu};
use ds_core::vocab::{RowState, Selection, Shown};
use ds_core::word::Word;
use std::sync::Arc;

/// How wide a thumbnail is drawn, in logical pixels. A windowed list needs every row's height up
/// front, and a page's height at a width the panel gives it is only known once the panel is
/// measured, so the width is fixed (it fits the narrowest the panel is dragged to) and the
/// thumbnail sits in the middle of the row.
const THUMB_WIDTH: f32 = 140.0;
/// The height of the page number under a thumbnail.
const NUMBER_HEIGHT: f32 = 20.0;
/// The room a row keeps around a thumbnail and its number.
const ROW_SPACE: f32 = 16.0;

fn go_to(page: PageIndex) -> StageIn {
    StageIn::Pdf(PdfIn::GoTo(Destination {
        page,
        offset: Permille(0),
    }))
}

/// A thumbnail's height at [`THUMB_WIDTH`], keeping the page's proportions.
fn thumb_height(doc: &PdfDoc, at: u32) -> f32 {
    let size = doc.size_of(PageIndex(at));
    let ratio = size.height.0 as f32 / size.width.0.max(1) as f32;
    let ratio = if ratio.is_finite() { ratio } else { 1.0 };
    THUMB_WIDTH * ratio.clamp(0.1, 10.0)
}

#[component]
pub(super) fn Thumbnails(doc: Held<PdfDoc>, cx: StageCx) -> Element {
    let reader = page_view(&cx.stage).map_or(PageIndex(0), |view| view.page);
    let (pdf, work, ticket, send) = (cx.pdf, cx.work, cx.ticket, cx.send);
    let held = Arc::clone(&doc.0);
    let heights = Arc::clone(&doc.0);
    let height =
        use_callback(move |at: u32| Px(thumb_height(&heights, at) + NUMBER_HEIGHT + ROW_SPACE));
    let row = use_callback(move |at: u32| {
        let current = if at == reader.0 {
            Emphasis::Current
        } else {
            Emphasis::Other
        };
        rsx! {
            Thumb {
                page: at,
                doc: Held(Arc::clone(&held)),
                current,
                pdf,
                work,
                ticket,
                send,
            }
        }
    });
    let keys: Vec<u32> = (0..doc.0.pages().get()).collect();
    rsx! {
        div { class: "viewer-thumbs",
            VirtualList::<u32> {
                label: "Pages",
                keys,
                row,
                height: RowHeight::PerKey(height),
                cursor: Some(reader.0),
            }
        }
    }
}

/// One page's thumbnail, asked of a worker once its row is mounted. A row that scrolls away
/// unmounts, and the texture a worker made stays on the shelf.
#[component]
fn Thumb(
    page: u32,
    doc: Held<PdfDoc>,
    current: Emphasis,
    pdf: PdfShelf,
    work: EventHandler<Job>,
    ticket: Ticket,
    send: EventHandler<StageIn>,
) -> Element {
    let gpu = use_gpu();
    let ready = gpu.device().is_some();
    let asking = Arc::clone(&doc.0);
    let asked = gpu.clone();
    use_effect(use_reactive!(|ready| {
        if ready && pdf.with_mut(|live| live.thumb_wanted(PageIndex(page))) {
            let ask = PdfAsk::Thumb {
                page: PageIndex(page),
            };
            let task = PdfTask::new(ticket, Arc::clone(&asking), asked.clone(), ask);
            work.call(Job::Pdf(task));
        }
    }));
    let texture = pdf.read().thumb(PageIndex(page)).cloned();
    let width = THUMB_WIDTH;
    let height = thumb_height(&doc.0, page);
    rsx! {
        div {
            class: "viewer-thumb",
            role: "option",
            "aria-label": format!("Page {}", page + 1),
            onclick: move |_| send.call(go_to(PageIndex(page))),
            span {
                class: "viewer-thumb-page",
                "data-mark": current.slug(),
                style: "width:{width}px;height:{height}px",
                TextureLayer { texture, fit: TextureFit::Fill }
                // A texture layer swallows a click, so the row around it would never hear one;
                // this cover is what the pointer lands on.
                span { class: "viewer-thumb-cover" }
            }
            span {
                class: "viewer-thumb-number",
                style: "height:{NUMBER_HEIGHT}px;line-height:{NUMBER_HEIGHT}px",
                "{page + 1}"
            }
        }
    }
}

/// The keys that move the outline, as Finder's and Preview's outlines take them: Left folds
/// (or steps out), Right unfolds (or steps in).
const OUTLINE_KEYS: [(Key, OutlineIn); 6] = [
    (Key::ArrowUp, OutlineIn::Move(RowStep::Up)),
    (Key::ArrowDown, OutlineIn::Move(RowStep::Down)),
    (Key::Home, OutlineIn::Move(RowStep::Top)),
    (Key::End, OutlineIn::Move(RowStep::Bottom)),
    (Key::ArrowLeft, OutlineIn::Fold),
    (Key::ArrowRight, OutlineIn::Unfold),
];

#[component]
pub(super) fn Outline(doc: Held<PdfDoc>, cx: StageCx) -> Element {
    let (pdf, send) = (cx.pdf, cx.send);
    let held = Arc::clone(&doc.0);
    let step = use_callback(move |input: OutlineIn| {
        pdf.with_mut(|live| live.fold_step(held.opening(), held.shape(), input));
    });
    let fold = pdf
        .read()
        .fold()
        .cloned()
        .unwrap_or_else(|| doc.0.opening().clone());
    let cursor = fold.cursor();
    let keys = fold.visible(doc.0.shape());
    let drawn = Arc::clone(&doc.0);
    let drawn_fold = fold.clone();
    let row = use_callback(move |at: u32| {
        let Some(entry) = drawn.outline.get(at as usize) else {
            return rsx! {};
        };
        let outline = match (drawn.shape().is_branch(at), drawn_fold.is_closed(at)) {
            (false, _) => RowOutline::Leaf,
            (true, true) => RowOutline::Branch(Shown::Hidden),
            (true, false) => RowOutline::Branch(Shown::Visible),
        };
        let state = RowState {
            selection: if Some(at) == cursor {
                Selection::Selected
            } else {
                Selection::Unselected
            },
            ..RowState::default()
        };
        let depth = entry.depth;
        let page = entry.page;
        let leads = if page.is_some() { "page" } else { "nowhere" };
        let content = rsx! {
            b {
                class: "ds-row-title viewer-outline-title ds-truncate",
                "data-leads": leads,
                style: "padding-left:calc(var(--s-16) * {depth})",
                "{entry.title}"
            }
        };
        rsx! {
            Row {
                title: "",
                size: RowSize::Compact,
                state,
                outline,
                content: Some(content),
                on_toggle: move |_| step.call(OutlineIn::Toggle(at)),
                onclick: move |_| {
                    step.call(OutlineIn::Select(at));
                    if let Some(page) = page {
                        send.call(go_to(page));
                    }
                },
            }
        }
    });
    let picked = Arc::clone(&doc.0);
    let onkeydown = move |event: KeyboardEvent| {
        if !event.modifiers().is_empty() {
            return;
        }
        let key = event.key();
        if key == Key::Enter {
            let page = cursor
                .and_then(|at| picked.outline.get(at as usize))
                .and_then(|entry| entry.page);
            if let Some(page) = page {
                event.prevent_default();
                event.stop_propagation();
                send.call(go_to(page));
            }
            return;
        }
        let Some(&(_, input)) = OUTLINE_KEYS.iter().find(|(named, _)| *named == key) else {
            return;
        };
        // The window would read the same key as a scroll or a step to another file.
        event.prevent_default();
        event.stop_propagation();
        step.call(input);
    };
    rsx! {
        div { class: "viewer-outline", role: "tree", onkeydown,
            VirtualList::<u32> {
                label: "Contents",
                keys,
                row,
                height: RowHeight::Fixed(Px(compact_px())),
                cursor,
            }
        }
    }
}
