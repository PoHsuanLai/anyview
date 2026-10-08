//! The picture reaches the screen through the window's device: the pane is drawn by the hybrid
//! painter and the pixels read back. Skips, with a note, where no GPU adapter opens (CI without one).

#![cfg(feature = "pane")]
// Helpers in an integration test crate are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use anyview_peek::{AnyPeeked, Body, Pane, peek};
use dioxus::prelude::*;
use ds::prelude::{Appearance, Ds, Material};
use ds_harness::{Backdrop, Backend, Harness, HarnessConfig, Query, Viewport};
use std::sync::Arc;
use support::{Home, fixture, pane_budget};

const VIEW: Viewport = Viewport {
    width: 360,
    height: 700,
    scale_percent: 100,
};

fn app() -> Element {
    let peeked = use_context::<Arc<AnyPeeked>>();
    rsx! {
        Ds {
            appearance: Appearance::default(),
            material: Material::Window,
            Pane { peeked }
        }
    }
}

#[test]
fn a_picture_is_uploaded_to_the_device_and_drawn_from_it() {
    let (src, sniffed) = fixture(Home::Image, "quadrants.png");
    let peeked = Arc::new(peek(&src, &sniffed, &pane_budget()));
    let Body::Picture(image) = &peeked.body else {
        panic!("a png peeks to a Picture body");
    };
    let size = image.picture.size();
    let config = HarnessConfig::new(VIEW)
        .with_backend(Backend::Hybrid)
        .with_context(peeked.clone());
    let mut harness = match Harness::try_new(app, config) {
        Ok(harness) => harness,
        Err(error) => {
            eprintln!("skipped: no GPU ({error})");
            return;
        }
    };
    // The device arrives with the first frame and the picture is written on the render after it.
    harness.render_over(Backdrop::Clear).unwrap();
    let shot = harness.render_over(Backdrop::Clear).unwrap();
    assert_eq!(
        harness.attr(".anyview-picture", "data-state").as_deref(),
        Some("ready")
    );
    let rect = harness.rect(".anyview-picture").unwrap();
    // The four corners of the box's picture hold the four quadrants' colours.
    let (w, h) = (f64::from(size.width.0), f64::from(size.height.0));
    let scale = (f64::from(rect.size.width.0) / w).min(f64::from(rect.size.height.0) / h);
    let (drawn_w, drawn_h) = (w * scale, h * scale);
    let (left, top) = (
        f64::from(rect.origin.x.0) + (f64::from(rect.size.width.0) - drawn_w) / 2.0,
        f64::from(rect.origin.y.0) + (f64::from(rect.size.height.0) - drawn_h) / 2.0,
    );
    let at = |fx: f64, fy: f64| {
        shot.get_pixel((left + drawn_w * fx) as u32, (top + drawn_h * fy) as u32)
            .0
    };
    let picture = image.picture.bytes();
    let texel = |x: u32, y: u32| {
        let at = ((y * size.width.0 + x) * 4) as usize;
        [
            picture[at],
            picture[at + 1],
            picture[at + 2],
            picture[at + 3],
        ]
    };
    let (qx, qy) = (size.width.0 / 4, size.height.0 / 4);
    let (rx, ry) = (size.width.0 * 3 / 4, size.height.0 * 3 / 4);
    assert_close(at(0.25, 0.25), texel(qx, qy));
    assert_close(at(0.75, 0.25), texel(rx, qy));
    assert_close(at(0.25, 0.75), texel(qx, ry));
    assert_close(at(0.75, 0.75), texel(rx, ry));
}

/// Within a few levels per channel: the picture was resampled to the box on the GPU.
fn assert_close(got: [u8; 4], want: [u8; 4]) {
    let near = got
        .iter()
        .zip(want)
        .all(|(got, want)| got.abs_diff(want) <= 6);
    assert!(near, "drew {got:?}, the peek holds {want:?}");
}
