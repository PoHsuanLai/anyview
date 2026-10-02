//! What the room draws: for each page in view, its box and the tiles, marks and links inside it,
//! all placed by the scene. Reads the live state and the machine's hit; decides nothing.

use super::live::PdfLive;
use super::page::{Emphasis, LinkView, Mark, PageDraw, TileView};
use super::scene::{Frame, Scene, drawn, percent};
use crate::HitIndex;
use anyview_core::PageIndex;
use anyview_pdf::{PageRect, ZoomBucket};

/// A box's place as CSS: `left`, `top`, `width` and `height` in pixels.
fn pixels(left: f32, top: f32, width: f32, height: f32) -> String {
    format!("left:{left:.2}px;top:{top:.2}px;width:{width:.2}px;height:{height:.2}px")
}

/// A rectangle of a page as CSS percentages of the page's box.
fn fractions(rect: PageRect) -> String {
    let (left, top, width, height) = percent(rect);
    format!("left:{left:.1}%;top:{top:.1}%;width:{width:.1}%;height:{height:.1}%")
}

/// What is read to draw the room.
#[derive(Debug, Clone, Copy)]
pub(super) struct Drawing<'a> {
    pub scene: &'a Scene,
    pub live: &'a PdfLive,
    pub top: u64,
    pub pan: u32,
    /// The hit the machine is on.
    pub current: Option<HitIndex>,
}

impl Drawing<'_> {
    /// The pages the room shows, top to bottom.
    pub(super) fn pages(self) -> Vec<PageDraw> {
        let frame = self.scene.frame;
        let view = self.scene.view(self.top, self.pan);
        let bucket = ZoomBucket::containing(self.scene.scale());
        let keys = drawn(self.scene, view, bucket, &self.live.tiles().keys());
        let from = u64::try_from(view.top).unwrap_or(0);
        let to = u64::try_from(view.bottom).unwrap_or(0);
        self.scene
            .layout
            .between(from, to)
            .map(|(page, place)| {
                let origin = (i64::from(place.left), i64::try_from(place.top).unwrap_or(0));
                let tiles = keys
                    .iter()
                    .filter(|key| key.page == page)
                    .filter_map(|key| {
                        let span = self.scene.tile_span(key)?;
                        let slot = self.live.tiles().get(key)?;
                        Some(TileView {
                            style: pixels(
                                frame.css(span.left - origin.0),
                                frame.css(span.top - origin.1),
                                frame.css(span.right - span.left),
                                frame.css(span.bottom - span.top),
                            ),
                            texture: slot.texture.clone(),
                        })
                    })
                    .collect();
                PageDraw {
                    number: page.0 + 1,
                    style: pixels(
                        frame.css(origin.0 - view.left),
                        frame.css(origin.1 - view.top),
                        frame.css(i64::from(place.size.width.0)),
                        frame.css(i64::from(place.size.height.0)),
                    ),
                    tiles,
                    marks: self.marks(page),
                    links: self.links(page),
                }
            })
            .collect()
    }

    fn marks(self, page: PageIndex) -> Vec<Mark> {
        self.live
            .hits()
            .iter()
            .enumerate()
            .filter(|(_, hit)| hit.page == page)
            .flat_map(|(at, hit)| {
                let emphasis =
                    if self.current == Some(HitIndex(u32::try_from(at).unwrap_or(u32::MAX))) {
                        Emphasis::Current
                    } else {
                        Emphasis::Other
                    };
                hit.rects.iter().map(move |rect| Mark {
                    style: fractions(*rect),
                    emphasis,
                })
            })
            .collect()
    }

    fn links(self, page: PageIndex) -> Vec<LinkView> {
        self.live.links(page).map_or_else(Vec::new, |links| {
            links
                .iter()
                .map(|link| LinkView {
                    style: fractions(link.rect),
                    target: link.target.clone(),
                })
                .collect()
        })
    }
}

/// The frame the room has before it is measured: nothing is drawn in it.
pub(super) const UNMEASURED: Frame = Frame {
    width: 1,
    height: 1,
    density: anyview_core::Permille(1000),
};
