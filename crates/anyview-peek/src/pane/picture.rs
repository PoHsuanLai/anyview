//! A downscaled picture, shown through quire's `TextureLayer`: the pixels go to the window's own
//! device and are drawn from there, never encoded to a PNG `data:` URL.

use anyview_image::ImagePeek;
use dioxus::prelude::*;
use ds::components::lists::preview::content::PANE_MEDIA;
use ds::components::overlays::skeleton::{Skeleton, SkeletonShape};
use ds::root::common::Common;
use ds_blitz::{PixelFormat, Pixels, TextureFit, TextureLayer, use_gpu};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

/// `image` fitted into the pane's media box. The device arrives with the window's first frame, so
/// until then (and on a renderer with no device, which draws no texture at all) the box shows a
/// placeholder block instead of a blank.
#[component]
pub(super) fn Picture(image: Arc<ImagePeek>, label: String) -> Element {
    let gpu = use_gpu();
    let texture = use_hook(|| gpu.handle());
    let uploaded = use_hook(|| Rc::new(RefCell::new(None::<Arc<ImagePeek>>)));
    let current = uploaded
        .borrow()
        .as_ref()
        .is_some_and(|done| Arc::ptr_eq(done, &image));
    let shown = if current || gpu.device().is_none() {
        current
    } else {
        // The device is there and this picture is not on it yet: write it once. A write happens in
        // a render because the handle is the layer's own, and replacing a texture redraws by itself.
        let size = image.picture.size();
        let uploaded_now = Pixels::new(
            PixelFormat::Rgba8Straight,
            size.width.0,
            size.height.0,
            image.picture.bytes(),
        )
        .ok()
        .and_then(|pixels| texture.update(&pixels).ok())
        .is_some();
        if uploaded_now {
            *uploaded.borrow_mut() = Some(image.clone());
        }
        uploaded_now
    };
    let room = format!(
        "width:{}px;height:{}px",
        PANE_MEDIA.width.0, PANE_MEDIA.height.0
    );
    rsx! {
        div { class: "anyview-picture", "data-state": if shown { "ready" } else { "pending" }, style: room,
            TextureLayer {
                texture: Some(texture.clone()),
                fit: TextureFit::Contain,
                common: Common {
                    aria_label: Some(label),
                    ..Common::default()
                },
            }
            if !shown {
                div { class: "anyview-picture-pending",
                    Skeleton { shape: SkeletonShape::Block }
                }
            }
        }
    }
}
