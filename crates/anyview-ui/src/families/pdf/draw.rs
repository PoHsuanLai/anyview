//! What the room draws: for each page in view, its box and the tiles, marks and links inside it,
//! all placed by the scene. Reads the live state and the machine's hit; decides nothing.

use super::fields::{FieldState, FieldView};
use super::inks::{Ink, InkState, inks_of, selection};
use super::live::PdfLive;
use super::page::{Emphasis, LinkView, Mark, PageDraw, TileView};
use super::scene::{Frame, Scene, drawn, percent};
use super::write::Writing;
use crate::{Draft, HitIndex, PdfMarkup};
use anyview_core::{MarkShape, MarkTool, NOTE_SIZE, PageIndex, PagePoint, PageRect, Permille};
use anyview_pdf::ZoomBucket;

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
    /// What has been written on the document.
    pub markup: &'a PdfMarkup,
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
                let size = (
                    frame.css(i64::from(place.size.width.0)),
                    frame.css(i64::from(place.size.height.0)),
                );
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
                    fields: self.fields(page),
                    inks: self.inks(page, size),
                    writing: self.writing(page),
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

    /// The fields on `page` the person can fill, each as it is now.
    fn fields(self, page: PageIndex) -> Vec<FieldView> {
        let markup = self.markup;
        markup
            .widgets()
            .iter()
            .enumerate()
            .filter(|(index, widget)| widget.page == page && markup.takes_input(*index))
            .map(|(index, widget)| {
                let value = markup.value_of(widget).to_owned();
                let state = if markup.focus() == Some(index) {
                    FieldState::Held
                } else if value != widget.value {
                    FieldState::Filled
                } else {
                    FieldState::Idle
                };
                FieldView {
                    index,
                    style: fractions(widget.rect),
                    on: markup.is_on(index),
                    widget: widget.clone(),
                    value,
                    state,
                }
            })
            .collect()
    }

    /// The marks on `page`, the words selected and the shape being drawn, under the order they
    /// were made in.
    fn inks(self, page: PageIndex, size: (f32, f32)) -> Vec<Ink> {
        let markup = self.markup;
        let mut inks: Vec<Ink> = markup
            .marks()
            .iter()
            .enumerate()
            .filter(|(_, mark)| mark.page == page)
            .flat_map(|(index, mark)| {
                let state = if markup.picked() == Some(index) {
                    InkState::Picked
                } else {
                    InkState::Resting
                };
                inks_of(mark, size, state)
            })
            .collect();
        inks.extend(selection(markup.selection(page)));
        if let Some(draft) = markup.draft().filter(|draft| draft.page == page)
            && let Some(shape) = shape_of(draft)
        {
            let ghost = anyview_core::Mark {
                page,
                color: markup.color(),
                shape,
                existing: None,
            };
            inks.extend(inks_of(&ghost, size, InkState::Draft));
        }
        inks
    }

    /// The note or text box being typed in, when it is on `page`.
    fn writing(self, page: PageIndex) -> Option<Writing> {
        let markup = self.markup;
        let index = markup.writing()?;
        let mark = markup.marks().get(index).filter(|mark| mark.page == page)?;
        let text = markup.words_of(index)?.to_owned();
        let (label, rect) = match &mark.shape {
            MarkShape::TextBox { rect, .. } => ("Text box", *rect),
            MarkShape::Note { at, .. } => {
                let top = Permille((at.y.0 + NOTE_SIZE.1).min(950));
                let right = Permille((at.x.0 + 300).min(1000));
                (
                    "Note",
                    PageRect {
                        left: at.x,
                        top,
                        right,
                        bottom: Permille((top.0 + 60).min(1000)),
                    },
                )
            }
            MarkShape::Highlight(_)
            | MarkShape::Underline(_)
            | MarkShape::Strikethrough(_)
            | MarkShape::Rectangle(_)
            | MarkShape::Oval(_)
            | MarkShape::Line { .. }
            | MarkShape::Arrow { .. }
            | MarkShape::Ink(_) => return None,
        };
        Some(Writing {
            label,
            style: fractions(rect),
            text,
        })
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

/// The shape a draft will be once the pointer is let go; `None` for a tool that draws no shape.
fn shape_of(draft: Draft) -> Option<MarkShape> {
    let (from, to): (PagePoint, PagePoint) = (draft.from, draft.to);
    match draft.tool {
        MarkTool::Rectangle => Some(MarkShape::Rectangle(PageRect::spanning(from, to))),
        MarkTool::Oval => Some(MarkShape::Oval(PageRect::spanning(from, to))),
        MarkTool::Line => Some(MarkShape::Line { from, to }),
        MarkTool::Arrow => Some(MarkShape::Arrow { from, to }),
        MarkTool::Sketch => Some(MarkShape::Ink(vec![draft.trail])),
        MarkTool::TextBox => Some(MarkShape::TextBox {
            rect: PageRect::spanning(from, to),
            text: String::new(),
        }),
        MarkTool::Pointer
        | MarkTool::Highlight
        | MarkTool::Underline
        | MarkTool::Strikethrough
        | MarkTool::Note => None,
    }
}
