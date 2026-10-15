//! The tree on screen: the visible nodes in reading order, each a row with its disclosure
//! triangle, indented by depth, its value coloured by type. Rows are read from the tree one at
//! a time as the list mounts them.

use super::super::places::NodePart;
use super::doc::TreeDoc;
use crate::families::inline_edit::InlineEdit;
use crate::families::rows::scaled_px;
use crate::families::view::{Held, StageCx};
use crate::families::{Mark, cut};
use crate::{
    AfterTyping, ContentScale, EditRevision, HitIndex, RowNo, Stage, StageIn, TreeIn, TypingIn,
    TypingSeed, TypingState,
};
use anyview_text::{NodeKind, Openness, RowLabel, VisibleRow};
use dioxus::prelude::*;
use ds::components::lists::row::row::{Outline, Row};
use ds::components::lists::row::size::RowSize;
use ds::components::lists::virtual_list::{RowHeight, VirtualList};
use ds::prelude::Px;
use ds::root::common::Common;
use ds::root::pass_through::ExtraClass;
use ds_core::vocab::{RowState, Selection, Shown};
use std::sync::Arc;

/// How many rows a page up or down moves by: the room's height in rows, less two, so the row the
/// cursor left stays in view.
pub(super) fn page_of(area: Option<crate::Area>, scale: ContentScale) -> u32 {
    area.map_or(1, |area| {
        let rows = (area.size.height.0 / scaled_px(scale)).floor();
        // a count of rows on screen is far below u32's range, and a negative one is none
        (rows as u32).saturating_sub(2).max(1)
    })
}

/// The class a value is drawn in: the highlighter's token class, so a tree and the source view
/// agree on what a string or a number looks like. An object's or an array's is its count, which
/// is faint.
fn value_class(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::String => "tok-string",
        NodeKind::Number => "tok-number",
        NodeKind::True | NodeKind::False | NodeKind::Null => "tok-constant",
        NodeKind::Object | NodeKind::Array | NodeKind::Unreadable => "viewer-node-count",
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

/// The marks the find has on one node's row: which part, where in its text, and whether it is the
/// hit the reader is on.
type NodeMarks = Vec<(NodePart, std::ops::Range<usize>, Mark)>;

/// The marks of `part`, as `cut` takes them.
fn marks_of(marks: &NodeMarks, part: NodePart) -> Vec<(std::ops::Range<usize>, Mark)> {
    marks
        .iter()
        .filter(|(at, _, _)| *at == part)
        .map(|(_, range, mark)| (range.clone(), *mark))
        .collect()
}

/// The part of a row being typed into, and the text it starts with.
struct Typed {
    part: NodePart,
    seed: String,
}

/// The box a value or a key is typed into.
fn editor(seed: &str, send: EventHandler<StageIn>) -> Element {
    let typing = EventHandler::new(move |input: TypingIn| {
        send.call(StageIn::Tree(TreeIn::Typing(input)));
    });
    rsx! {
        InlineEdit {
            seed: seed.to_owned(),
            on_return: AfterTyping::Stay,
            on_tab: None,
            send: typing,
        }
    }
}

fn node_row(
    visible: &VisibleRow,
    state: RowState,
    send: EventHandler<StageIn>,
    index: u32,
    (marks, typed): (&NodeMarks, Option<&Typed>),
) -> Element {
    let toggled = visible.path.clone();
    let depth = visible.depth;
    let key = label_text(&visible.row.label);
    let class = value_class(visible.row.kind);
    let key_pieces = cut(&key, &marks_of(marks, NodePart::Key));
    let value_pieces = cut(&visible.row.preview, &marks_of(marks, NodePart::Value));
    let typing_key = typed.filter(|typed| typed.part == NodePart::Key);
    let typing_value = typed.filter(|typed| typed.part == NodePart::Value);
    let content = rsx! {
        span {
            class: "viewer-node",
            style: "padding-left:calc(var(--s-16) * {depth} * var(--zoom))",
            span { class: "tok-plain viewer-node-key",
                if let Some(typed) = typing_key {
                    {editor(&typed.seed, send)}
                } else {
                    for (piece , mark) in key_pieces {
                        span { class: mark.class(), "{piece}" }
                    }
                }
            }
            span { class: "tok-punctuation", ": " }
            span { class: "{class} viewer-node-value ds-truncate",
                if let Some(typed) = typing_value {
                    {editor(&typed.seed, send)}
                } else {
                    for (piece , mark) in value_pieces {
                        span { class: mark.class(), "{piece}" }
                    }
                }
            }
        }
    };
    rsx! {
        Row {
            title: "",
            size: RowSize::Compact,
            state,
            outline: outline(visible.openness),
            content: Some(content),
            common: Common {
                extra_class: ExtraClass::parse("viewer-node-row").ok(),
                ..Common::default()
            },
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
        return rsx! { div { class: "viewer-data viewer-scrolls" } };
    };
    let open = stage.open().clone();
    let tree = doc.0.now();
    let total = tree.visible_count(&open);
    let cursor = stage.row().map(|row| row.0);
    let scale = stage.scale();
    // The hit the find is on, so its row is marked apart from the others.
    let current = stage.find().and_then(|find| find.hits.current());
    let marked = cx.hits.clone();
    let send = cx.send;
    let held = Arc::clone(&doc.0);
    let drawn_open = open.clone();
    // The rows are drawn again when the edits change or something is typed in, though the cursor
    // stays where it was.
    let typing = stage.edits().typing.clone();
    let revision = stage.edits().log.revision;
    let mut drawn = use_signal(|| (revision, typing.clone()));
    use_effect(use_reactive!(|revision, typing| {
        drawn.set((revision, typing));
    }));
    let seat = cursor;
    let row = use_callback(move |index: u32| {
        let (_, typing): (EditRevision, TypingState) = drawn.read().clone();
        let state = RowState {
            selection: if Some(index) == cursor {
                Selection::Selected
            } else {
                Selection::Unselected
            },
            ..RowState::default()
        };
        match held
            .now()
            .visible(&drawn_open, index..index.saturating_add(1))
            .first()
        {
            Some(visible) => {
                let marks: NodeMarks = match marked.as_ref() {
                    Some(found) => {
                        let (first, on_node) = found.0.at_node(&visible.path);
                        on_node
                            .iter()
                            .enumerate()
                            .map(|(at, hit)| {
                                let this =
                                    HitIndex(first.0 + u32::try_from(at).unwrap_or(u32::MAX));
                                let mark = if current == Some(this) {
                                    Mark::Current
                                } else {
                                    Mark::Hit
                                };
                                (hit.part, hit.from as usize..hit.to as usize, mark) // a u32 fits a usize
                            })
                            .collect()
                    }
                    None => Vec::new(),
                };
                let typed = match (Some(index) == seat, &typing) {
                    (true, TypingState::Value(seed)) => Some(Typed {
                        part: NodePart::Value,
                        seed: seed_text(&held, visible, seed, NodePart::Value),
                    }),
                    (true, TypingState::Key) => Some(Typed {
                        part: NodePart::Key,
                        seed: seed_text(&held, visible, &TypingSeed::Existing, NodePart::Key),
                    }),
                    (false, _) | (true, TypingState::Idle) => None,
                };
                node_row(visible, state, send, index, (&marks, typed.as_ref()))
            }
            None => rsx! {},
        }
    });
    let keys: Vec<u32> = (0..total).collect();
    rsx! {
        div {
            class: "viewer-data viewer-scrolls",
            "data-body": "tree",
            style: "--zoom:{scale.factor()}",
            VirtualList::<u32> {
                label: "Nodes",
                keys,
                row,
                height: RowHeight::Fixed(Px(scaled_px(scale))),
                cursor,
                onselect: move |index: u32| send.call(StageIn::Tree(TreeIn::Select(RowNo(index)))),
            }
        }
    }
}

/// The text a node's value or key starts with while it is typed into: what the person typed to
/// begin, else the whole value (a string's characters, a number as written) or the whole key.
fn seed_text(doc: &TreeDoc, visible: &VisibleRow, seed: &TypingSeed, part: NodePart) -> String {
    if let TypingSeed::Typed(text) = seed {
        return text.as_str().to_owned();
    }
    let held = doc.session();
    let found = held.as_ref().and_then(|session| match part {
        NodePart::Value => session.value_text(&visible.path),
        NodePart::Key => session.key_text(&visible.path),
    });
    found.unwrap_or_default()
}
