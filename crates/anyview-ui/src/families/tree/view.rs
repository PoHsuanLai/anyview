//! The tree on screen: the visible nodes in reading order, each a row with its disclosure
//! triangle, indented by depth, its value coloured by type. Rows are read from the tree one at
//! a time as the list mounts them.

use super::doc::TreeDoc;
use crate::families::view::{Held, StageCx};
use crate::{RowNo, Stage, StageIn, TreeIn};
use anyview_text::{NodeKind, Openness, RowLabel, VisibleRow};
use dioxus::prelude::*;
use ds::components::lists::row::row::{Outline, Row};
use ds::components::lists::row::size::RowSize;
use ds::components::lists::virtual_list::{RowHeight, VirtualList};
use ds::prelude::Px;
use ds_core::vocab::{RowState, Selection, Shown};
use std::sync::Arc;

/// The height of a compact row.
const ROW_PX: f32 = 24.0;
/// How many rows a page up or down moves by: the room's height in rows, less two, so the row the
/// cursor left stays in view.
pub(super) fn page_of(area: Option<crate::Area>) -> u32 {
    area.map_or(1, |area| {
        let rows = (area.size.height.0 / ROW_PX).floor();
        // a count of rows on screen is far below u32's range, and a negative one is none
        (rows as u32).saturating_sub(2).max(1)
    })
}

/// How far each level of depth indents a row.
const INDENT_PX: f32 = 16.0;

/// The token class a value is coloured by: the highlighter's own, so a tree and the source view
/// agree on what a string or a number looks like.
fn value_class(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::String => "tok-string",
        NodeKind::Number => "tok-number",
        NodeKind::True | NodeKind::False | NodeKind::Null => "tok-constant",
        NodeKind::Object | NodeKind::Array => "tok-comment",
    }
}

fn label_text(label: &RowLabel) -> String {
    match label {
        RowLabel::Root => "(root)".to_owned(),
        RowLabel::Key(key) => key.clone(),
        RowLabel::Index(index) => index.to_string(),
    }
}

fn outline(openness: Openness) -> Outline {
    match openness {
        Openness::Leaf => Outline::Leaf,
        Openness::Closed => Outline::Branch(Shown::Hidden),
        Openness::Open => Outline::Branch(Shown::Visible),
    }
}

fn node_row(
    visible: &VisibleRow,
    state: RowState,
    send: EventHandler<StageIn>,
    index: u32,
) -> Element {
    let toggled = visible.path.clone();
    let depth = visible.depth as f32; // a depth is far below f32's exact range
    let key = label_text(&visible.row.label);
    let class = value_class(visible.row.kind);
    let preview = visible.row.preview.clone();
    let content = rsx! {
        span {
            class: "viewer-node",
            style: "padding-left:{depth * INDENT_PX}px",
            span { class: "tok-type viewer-node-key", "{key}" }
            span { class: "tok-punctuation", ": " }
            span { class: "{class} viewer-node-value ds-truncate", "{preview}" }
        }
    };
    rsx! {
        Row {
            title: "",
            size: RowSize::Compact,
            state,
            outline: outline(visible.openness),
            content: Some(content),
            on_toggle: move |_| send.call(StageIn::Tree(TreeIn::Toggle(toggled.clone()))),
            onclick: move |_| {
                send.call(StageIn::Tree(TreeIn::Select(RowNo(index))));
            },
        }
    }
}

#[component]
pub(super) fn TreeContent(doc: Held<TreeDoc>, cx: StageCx) -> Element {
    let Stage::Tree(stage) = &cx.stage else {
        return rsx! { div { class: "viewer-data" } };
    };
    let open = stage.open().clone();
    let total = doc.0.tree.visible_count(&open);
    let cursor = stage.row().map(|row| row.0);
    let send = cx.send;
    let held = Arc::clone(&doc.0);
    let drawn_open = open.clone();
    let row = use_callback(move |index: u32| {
        let state = RowState {
            selection: if Some(index) == cursor {
                Selection::Selected
            } else {
                Selection::Unselected
            },
            ..RowState::default()
        };
        match held
            .tree
            .visible(&drawn_open, index..index.saturating_add(1))
            .first()
        {
            Some(visible) => node_row(visible, state, send, index),
            None => rsx! {},
        }
    });
    let keys: Vec<u32> = (0..total).collect();
    let note = (doc.0.coverage == anyview_text::Coverage::Prefix)
        .then(|| "Showing the start of the file".to_owned());
    rsx! {
        div { class: "viewer-data", "data-body": "tree",
            VirtualList::<u32> {
                label: "Nodes",
                keys,
                row,
                height: RowHeight::Fixed(Px(ROW_PX)),
                cursor,
                onselect: move |index: u32| send.call(StageIn::Tree(TreeIn::Select(RowNo(index)))),
            }
            if let Some(note) = note {
                p { class: "viewer-data-note", "{note}" }
            }
        }
    }
}
