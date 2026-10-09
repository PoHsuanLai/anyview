//! A preview may be shared or screenshotted, so it never says where a photo was taken, and it
//! never shows the serial numbers a camera writes. The viewer's own Info panel may list the place
//! (`anyview_image::picture_facts`); the launcher's pane, the peek's facts and every pane part may
//! not. The fixtures are made by `anyview-image/tests/fixtures/make_located.py`.

#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::{FactGroup, FactLabel};
use anyview_peek::peek;
use support::{Home, fixture, pane_budget};

/// What no preview says of a located photo.
const WITHHELD: [&str; 11] = [
    "37.7749",
    "122.4194",
    "37.77",
    "°",
    "37774900",
    "122419400",
    "Coordinate",
    "location",
    "SN-BODY",
    "SN-LENS",
    "123456",
];

#[test]
fn the_peek_lists_the_camera_but_neither_the_place_nor_a_serial_number() {
    for name in ["located.jpg", "located.png"] {
        let (src, sniffed) = fixture(Home::Image, name);
        let peeked = peek(&src, &sniffed, &pane_budget());
        assert!(
            peeked
                .facts
                .rows()
                .iter()
                .any(|row| row.label == FactLabel::Camera),
            "{name}: the camera is listed"
        );
        for row in peeked.facts.rows() {
            assert_ne!(row.group, FactGroup::Location, "{name}: {:?}", row.label);
            assert!(
                ![FactLabel::Coordinates, FactLabel::Altitude].contains(&row.label),
                "{name}: {:?}",
                row.label
            );
        }
        // The whole value a worker hands to sill, mailo and temor: the body's picture and its
        // EXIF too, not only the rows.
        let everything = format!("{peeked:?}");
        for needle in WITHHELD {
            assert!(
                !everything.contains(needle),
                "{name}: the peek holds {needle:?}"
            );
        }
    }
}

#[cfg(feature = "pane")]
mod pane {
    use super::*;
    use anyview_peek::{AnyPeeked, Pane};
    use dioxus::core::VirtualDom;
    use dioxus::prelude::*;
    use ds::assembly::ds::Inject;
    use ds::prelude::{Appearance, Ds, Material};
    use std::sync::Arc;

    #[derive(Props, Clone, PartialEq)]
    struct Setup {
        peeked: Arc<AnyPeeked>,
    }

    #[allow(non_snake_case)]
    fn Root(setup: Setup) -> Element {
        let peeked = setup.peeked;
        rsx! {
            Ds {
                appearance: Appearance::default(),
                material: Material::Window,
                stylesheet: Inject::Host,
                Pane { peeked }
            }
        }
    }

    #[test]
    fn the_pane_draws_neither_the_place_nor_a_serial_number() {
        for name in ["located.jpg", "located.png"] {
            let (src, sniffed) = fixture(Home::Image, name);
            let peeked = Arc::new(peek(&src, &sniffed, &pane_budget()));
            let mut dom = VirtualDom::new_with_props(Root, Setup { peeked });
            dom.rebuild_in_place();
            let html = dioxus_ssr::render(&dom);
            assert!(html.contains("TestCam"), "{name}: the camera is drawn");
            for needle in WITHHELD {
                assert!(!html.contains(needle), "{name}: the pane draws {needle:?}");
            }
        }
    }
}
