//! The pane: what a peek looks like. One component draws every kind from the type-erased
//! [`AnyPeeked`]: a picture, a page, highlighted lines, a table, a tree's top level, a rendered
//! Markdown start, a folder or a file's facts, over the file's name and its rows of facts.
//!
//! It is drawn with quire's components (`FactList`, `Table`, `PdfThumb`, `TextureLayer`,
//! `InlineBanner`) and a small stylesheet of its own in the `app` layer, made of design-system
//! tokens only. The pane never takes the keyboard and holds no state of its own beyond the texture
//! it shows: the actions under it are the host's (the launcher's rows, the viewer's capsule).

mod frame;
mod grid;
mod lines;
pub(crate) mod page;
mod parts;
mod picture;
mod specimen;

use crate::any::AnyPeeked;
use crate::body::Body;
use dioxus::prelude::*;
use ds::components::content::pdf_thumb::PdfThumb;
use ds::components::fields::fact_list::{Fact, FactList};
use ds::components::lists::preview::content::PANE_MEDIA;
use ds::prelude::{
    AppStyle, Icon, IconSize, IconSource, IconView, InlineBanner, Severity, Size, Word,
};
use ds::root::common::Common;
use ds::style::icon::family::PlateFamily;
use ds::style::icon::render::IconPx;
use std::sync::Arc;

pub use parts::{Part, Parts};

// The page is rasterised for the box the pane fits it into; the two must stay one size.
const _: () = {
    let mine = crate::pdf::raster::PANE_MEDIA;
    assert!(mine.width.0 == PANE_MEDIA.width.0 && mine.height.0 == PANE_MEDIA.height.0);
};

/// The pane's stylesheet: the `app` layer, tokens only (`tests/coherence.rs` lints it Strict).
pub const STYLE: &str = include_str!("pane.css");

/// What a peeked file looks like. Draw it inside a `Ds` root, in a column as wide as the pane
/// (328 px of content is what `PANE_MEDIA` fits a picture or a page into).
///
/// `parts` says which of the media, the name and the facts are drawn; the default is all three. A
/// strip that shows an attachment as a picture alone asks for `Parts::of([Part::Media])`.
///
/// `peeked` is shared so a host can keep one result and hand it to the pane on every render
/// without copying pixels or comparing them (`Arc` equality starts at the pointer).
#[component]
pub fn Pane(
    peeked: Arc<AnyPeeked>,
    #[props(default)] parts: Parts,
    #[props(default = PANE_MEDIA)] page_room: Size,
    #[props(default)] common: Common,
) -> Element {
    let name = peeked.name.clone();
    let facts: Vec<Fact> = peeked
        .facts
        .rows()
        .iter()
        .map(|row| Fact::new(row.label.label(), row.value.as_str()))
        .collect();
    let data = common.data_attributes();
    rsx! {
        AppStyle { css: STYLE }
        div {
            id: common.id.clone(),
            class: common.class("anyview-pane"),
            role: "region",
            "aria-label": "{name} preview",
            "data-kind": peeked.kind.slug(),
            "data-body": peeked.body.slug(),
            ..data,
            if parts.contains(Part::Media) {
                div { class: "anyview-pane-media", {media(&peeked, page_room)} }
            }
            if parts.contains(Part::Title) {
                b { class: "anyview-pane-title", "{name}" }
            }
            if parts.contains(Part::Facts) {
                div { class: "anyview-pane-facts", FactList { facts } }
            }
        }
    }
}

/// The media box's contents for `peeked`'s body.
fn media(peeked: &Arc<AnyPeeked>, page_room: Size) -> Element {
    match &peeked.body {
        Body::Picture(image) => rsx! {
            picture::Picture { image: image.clone(), label: peeked.name.clone() }
        },
        Body::Page(page) => rsx! {
            PdfThumb { page: page::thumb(&page.page), size: page_room, label: peeked.name.clone() }
        },
        Body::Plain(plain) => lines::plain(&plain.lines),
        Body::Code(code) => lines::code(&code.lines),
        Body::Markdown(markdown) => rsx! {
            frame::Frame { html: markdown.html.clone() }
        },
        Body::Table(table) => grid::table(table),
        Body::Archive(archive) => grid::archive(archive),
        Body::Font(font) => match specimen::lines(font) {
            Some(lines) => lines,
            None => plate(Icon::File, PlateFamily::Blue),
        },
        Body::Tree(tree) => grid::tree(tree),
        Body::Folder(_) => plate(Icon::Folder, PlateFamily::Blue),
        Body::FactsOnly(_) => plate(Icon::File, PlateFamily::Blue),
        Body::Unavailable(reason) => rsx! {
            {plate(Icon::File, PlateFamily::Red)}
            InlineBanner { severity: Severity::Info, text: reason.clone() }
        },
    }
}

/// A glyph on a plate: what stands for a file with nothing to draw.
fn plate(glyph: Icon, family: PlateFamily) -> Element {
    rsx! {
        IconView {
            source: IconSource::Glyph(glyph),
            size: IconSize::Px(IconPx(64)),
            plate: Some(family),
        }
    }
}
