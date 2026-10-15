//! One page on screen: a box at the page's place holding the tiles drawn of it, the marks of the
//! search over it and the links on it. The tiles are `TextureLayer`s, so a tile is the texture a
//! worker uploaded and nothing about its pixels passes through here; a mark and a link sit at
//! fractions of the page, so neither depends on the zoom.

use super::fields::{FieldView, FormField};
use super::inks::Ink;
use super::write::{WriteBox, Writing};
use crate::MarkupIn;
use anyview_pdf::LinkTarget;
use dioxus::prelude::*;
use ds_blitz::{Sampling, TextureFit, TextureHandle, TextureLayer};
use ds_core::word::Word;

/// How strongly a hit is marked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub(super) enum Emphasis {
    /// The hit the machine is on.
    Current,
    /// Any other hit.
    Other,
}

/// A tile placed in its page's box.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct TileView {
    /// `left`, `top`, `width` and `height` in the page box's CSS pixels.
    pub style: String,
    pub texture: TextureHandle,
}

/// A hit placed over its page, at percentages of the box.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Mark {
    pub style: String,
    pub emphasis: Emphasis,
}

/// A link placed over its page, at percentages of the box.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct LinkView {
    pub style: String,
    pub target: LinkTarget,
}

/// A page's box and what is drawn in it.
#[component]
pub(super) fn PageBox(
    /// The page's number from one, for the box's name.
    number: u32,
    /// The box's place in the room.
    style: String,
    tiles: Vec<TileView>,
    marks: Vec<Mark>,
    links: Vec<LinkView>,
    /// A link was clicked.
    onlink: EventHandler<LinkTarget>,
    /// The form's fields on this page.
    #[props(default)]
    fields: Vec<FieldView>,
    /// What the person wrote on this page: marks, the selection, the shape being drawn.
    #[props(default)]
    inks: Vec<Ink>,
    /// The note or text box being typed in, when it is on this page.
    #[props(default)]
    writing: Option<Writing>,
    /// An input for the markup.
    #[props(default)]
    onmark: EventHandler<MarkupIn>,
) -> Element {
    rsx! {
        div {
            class: "viewer-pdf-page",
            role: "img",
            "aria-label": "Page {number}",
            "data-page": "{number}",
            style,
            for tile in tiles {
                div { class: "viewer-pdf-tile", style: tile.style,
                    TextureLayer {
                        texture: Some(tile.texture),
                        fit: TextureFit::Fill,
                        sampling: Sampling::Bicubic,
                    }
                }
            }
            for mark in marks {
                div {
                    class: "viewer-pdf-hit",
                    "data-hit": mark.emphasis.slug(),
                    style: mark.style,
                }
            }
            for link in links {
                {
                    let target = link.target.clone();
                    rsx! {
                        div {
                            class: "viewer-pdf-link",
                            role: "link",
                            style: link.style,
                            onclick: move |_| onlink.call(target.clone()),
                        }
                    }
                }
            }
            for ink in inks {
                div {
                    class: "viewer-pdf-ink",
                    "data-ink": ink.kind.slug(),
                    "data-state": ink.state.slug(),
                    style: ink.style,
                    if !ink.text.is_empty() {
                        span { class: "viewer-pdf-ink-text", "{ink.text}" }
                    }
                    if !ink.path.is_empty() {
                        // Drawn as quire's own vectors are: the stroke in the svg's attributes, its
                        // colour the box's `color` (the stylesheet does not reach inside an svg).
                        svg {
                            class: "viewer-pdf-sketch",
                            "data-ds-svg": "sketch",
                            "aria-hidden": "true",
                            view_box: "0 0 {ink.extent.0} {ink.extent.1}",
                            for stroke in ink.path.split('|') {
                                polyline {
                                    points: "{stroke}",
                                    fill: "none",
                                    stroke: "currentColor",
                                    "stroke-width": "2",
                                    "stroke-linecap": "round",
                                    "stroke-linejoin": "round",
                                }
                            }
                        }
                    }
                }
            }
            for field in fields {
                {
                    let index = field.index;
                    rsx! {
                        FormField { key: "{index}", view: field, mark: onmark }
                    }
                }
            }
            if let Some(writing) = writing {
                WriteBox { writing, mark: onmark }
            }
        }
    }
}

/// What a page's box is drawn from.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PageDraw {
    pub number: u32,
    pub style: String,
    pub tiles: Vec<TileView>,
    pub marks: Vec<Mark>,
    pub links: Vec<LinkView>,
    pub fields: Vec<FieldView>,
    pub inks: Vec<Ink>,
    pub writing: Option<Writing>,
}
