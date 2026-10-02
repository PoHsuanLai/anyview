//! Moving the view: where the room is in the stack of pages as the last input left it, and the
//! inputs that move it. The stage machine decides which page is at the top and the zoom; the
//! room's exact place within a thousandth of a page is the view's own, so a scroll smaller than
//! that still moves it and the machine hears only when the page or its offset changes.

use super::scene::{Scene, pinched};
use crate::{Destination, PdfIn, StageIn};
use anyview_core::{PageIndex, Permille, Zoom};
use anyview_pdf::PageRect;
use dioxus::prelude::*;
use std::borrow::Cow;

/// Where the room is: the top of it in the stack, how far it is panned across, and the scale the
/// stack is at, all in device pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Cursor {
    pub top: u64,
    pub pan: u32,
    pub scale: Permille,
}

/// What moves the view: the cursor, the stack it moves over and the way to tell the machine.
#[derive(Clone, Copy)]
pub(super) struct Steering {
    cursor: Signal<Cursor>,
    scene: Memo<Scene>,
    send: EventHandler<StageIn>,
}

impl Steering {
    pub(super) fn new(
        cursor: Signal<Cursor>,
        scene: Memo<Scene>,
        send: EventHandler<StageIn>,
    ) -> Steering {
        Steering {
            cursor,
            scene,
            send,
        }
    }

    /// Runs `read` with the stack at the cursor's scale: the memo's own unless an input that has
    /// not been drawn yet changed the scale.
    fn at_cursor<T>(self, read: impl FnOnce(&Scene, Cursor) -> T) -> T {
        let cursor = *self.cursor.peek();
        let held = self.scene.peek();
        let scene = if held.scale() == cursor.scale {
            Cow::Borrowed(&*held)
        } else {
            Cow::Owned(held.at_scale(cursor.scale))
        };
        read(&scene, cursor)
    }

    /// Settles the cursor and tells the machine where the room now is.
    fn settle(self, top: u64, pan: u32, scale: Permille, place: (PageIndex, Permille)) {
        let mut cursor = self.cursor;
        cursor.set(Cursor { top, pan, scale });
        self.send.call(StageIn::Pdf(PdfIn::Scroll {
            page: place.0,
            offset: place.1,
        }));
    }

    /// The content moved `dx` and `dy` device pixels, as a finger or a wheel moves it.
    pub(super) fn scroll_by(self, dx: i64, dy: i64) {
        let moved = self.at_cursor(|scene, cursor| {
            let top = (i64::try_from(cursor.top).unwrap_or(i64::MAX) - dy)
                .clamp(0, i64::try_from(scene.deepest()).unwrap_or(i64::MAX));
            let pan = (i64::from(cursor.pan) - dx).clamp(0, i64::from(scene.widest()));
            let top = u64::try_from(top).unwrap_or(0);
            (
                top,
                u32::try_from(pan).unwrap_or(0),
                cursor.scale,
                scene.place_at(top),
            )
        });
        self.settle(moved.0, moved.1, moved.2, moved.3);
    }

    /// Zoom by `by` thousandths about `at`, a point of the room in device pixels, which stays
    /// where it is under the pointer.
    pub(super) fn zoom_by(self, by: i32, at: (u32, u32)) {
        let zoomed = self.at_cursor(|scene, cursor| {
            let scale = pinched(cursor.scale, by);
            if scale == cursor.scale {
                return None;
            }
            let after = scene.at_scale(scale);
            let (top, pan) = after.anchored(scene, cursor.top, cursor.pan, at);
            Some((scale, top, pan, after.place_at(top)))
        });
        if let Some((scale, top, pan, place)) = zoomed {
            self.send
                .call(StageIn::Pdf(PdfIn::SetZoom(Zoom::scaled(scale))));
            self.settle(top, pan, scale, place);
        }
    }

    /// Go to `place`: the top of the room at the page and how far down it.
    pub(super) fn go_to(self, place: Destination) {
        let went = self.at_cursor(|scene, cursor| {
            let top = scene.top_of(place.page, place.offset);
            (top, cursor.pan, cursor.scale, scene.place_at(top))
        });
        self.settle(went.0, went.1, went.2, went.3);
    }

    /// Go to the hit on `page` marked by `rects`, so it shows in the room.
    pub(super) fn show(self, page: PageIndex, rects: &[PageRect]) {
        let went = self.at_cursor(|scene, cursor| {
            let top = scene.top_for_hit(page, rects)?;
            Some((
                top,
                scene.pan_for_hit(page, rects),
                cursor.scale,
                scene.place_at(top),
            ))
        });
        if let Some((top, pan, scale, place)) = went {
            self.settle(top, pan, scale, place);
        }
    }
}
