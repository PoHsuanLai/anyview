//! The pane drawn with only the parts a caller asked for: the default is the whole pane, and a
//! strip that wants the picture alone gets the media box and nothing else.

#![cfg(feature = "pane")]
#![allow(clippy::unwrap_used)]

mod support;

use anyview_peek::{AnyPeeked, Pane, Part, Parts, peek};
use dioxus::core::VirtualDom;
use dioxus::prelude::*;
use ds::assembly::ds::Inject;
use ds::prelude::{Appearance, Ds, Material};
use std::sync::Arc;
use support::{Home, fixture, pane_budget};

#[derive(Props, Clone, PartialEq)]
struct Setup {
    peeked: Arc<AnyPeeked>,
    parts: Option<Parts>,
}

#[allow(non_snake_case)]
fn Root(setup: Setup) -> Element {
    let peeked = setup.peeked;
    rsx! {
        Ds {
            appearance: Appearance::default(),
            material: Material::Window,
            stylesheet: Inject::Host,
            if let Some(parts) = setup.parts {
                Pane { peeked, parts }
            } else {
                Pane { peeked }
            }
        }
    }
}

fn render(parts: Option<Parts>) -> String {
    let (src, sniffed) = fixture(Home::Text, "sample.rs");
    let peeked = Arc::new(peek(&src, &sniffed, &pane_budget()));
    let mut dom = VirtualDom::new_with_props(Root, Setup { peeked, parts });
    dom.rebuild_in_place();
    dioxus_ssr::render(&dom)
}

#[test]
fn the_default_pane_is_every_part() {
    assert_eq!(render(None), render(Some(Parts::ALL)));
}

#[test]
fn each_part_is_drawn_only_when_asked_for() {
    // name, parts, media box, title, facts
    let cases: [(&str, Parts, bool, bool, bool); 4] = [
        ("all", Parts::ALL, true, true, true),
        ("media alone", Parts::of([Part::Media]), true, false, false),
        (
            "title and facts",
            Parts::of([Part::Title, Part::Facts]),
            false,
            true,
            true,
        ),
        ("nothing", Parts::of([]), false, false, false),
    ];
    for (name, parts, media, title, facts) in cases {
        let html = render(Some(parts));
        assert_eq!(
            html.contains("<div class=\"anyview-pane-media\""),
            media,
            "{name}: media"
        );
        assert_eq!(
            html.contains("<b class=\"anyview-pane-title\""),
            title,
            "{name}: title"
        );
        assert_eq!(
            html.contains("<div class=\"anyview-pane-facts\""),
            facts,
            "{name}: facts"
        );
        assert!(
            html.contains("<div class=\"anyview-pane\"") || html.contains("class=\"anyview-pane "),
            "{name}: the region is always there"
        );
    }
}
