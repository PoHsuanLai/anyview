//! The picture edits' transitions.

use super::crop::{CropAspect, CropBox};
use super::model::{Crop, Cropping, Held, PictureEditIn, PictureEditing, PictureEdits};
use anyview_core::{Adjust, PixelSize};

impl PictureEdits {
    /// The edits after `input`.
    pub fn step(mut self, input: PictureEditIn) -> PictureEdits {
        if let PictureEditIn::Landed { size, editing } = input {
            return PictureEdits {
                base: size,
                editing,
                ..PictureEdits::default()
            };
        }
        let Some(base) = self
            .base
            .filter(|_| self.editing == PictureEditing::Allowed)
        else {
            return self;
        };
        match input {
            PictureEditIn::Landed {
                size: _,
                editing: _,
            } => {}
            PictureEditIn::Turn(by) => self.change(base, self.now.turned(by)),
            PictureEditIn::Flip(axis) => self.change(base, self.now.flipped(axis)),
            PictureEditIn::Resize(size) => {
                let size = (size != self.now.turned_size(base)).then_some(size);
                self.change(base, Adjust { size, ..self.now });
            }
            PictureEditIn::CropOn => {
                if !self.is_cropping() {
                    self.restart_crop(base, CropAspect::default());
                }
            }
            PictureEditIn::CropOff => self.crop = Cropping::Off,
            PictureEditIn::Grab { at, reach } => {
                if let Cropping::On(crop) = &mut self.crop
                    && crop.held.is_none()
                    && let Some(grip) = crop.draft.grip_at(at, reach)
                {
                    crop.held = Some(Held {
                        grip,
                        from: crop.draft,
                        at,
                    });
                }
            }
            PictureEditIn::Drag(to) => {
                let shown = self.now.turned_size(base);
                if let Cropping::On(crop) = &mut self.crop
                    && let Some(held) = &crop.held
                {
                    let by = (
                        i64::from(to.x.0) - i64::from(held.at.x.0),
                        i64::from(to.y.0) - i64::from(held.at.y.0),
                    );
                    crop.draft = held
                        .from
                        .dragged(held.grip, by, shown, crop.aspect.ratio(shown));
                }
            }
            PictureEditIn::Release => {
                if let Cropping::On(crop) = &mut self.crop {
                    crop.held = None;
                }
            }
            PictureEditIn::Hold(aspect) => {
                let shown = self.now.turned_size(base);
                if let Cropping::On(crop) = &mut self.crop {
                    crop.aspect = aspect;
                    crop.held = None;
                    if let Some(ratio) = aspect.ratio(shown) {
                        crop.draft = crop.draft.shaped(ratio);
                    }
                }
            }
            PictureEditIn::Apply => self.apply(base),
            PictureEditIn::Discard => {
                self = PictureEdits {
                    base: self.base,
                    editing: self.editing,
                    ..PictureEdits::default()
                };
            }
            PictureEditIn::Undo => {
                if let Some(before) = self.undo.pop() {
                    self.redo.push(self.now);
                    self.now = before;
                    self.refresh_crop(base);
                }
            }
            PictureEditIn::Redo => {
                if let Some(after) = self.redo.pop() {
                    self.undo.push(self.now);
                    self.now = after;
                    self.refresh_crop(base);
                }
            }
        }
        self
    }

    /// `next` is what has been done now, and what was done is kept to take back. Doing the same
    /// again changes nothing, and anything that had been taken back can no longer be done again.
    fn change(&mut self, base: PixelSize, next: Adjust) {
        if next == self.now {
            return;
        }
        self.undo.push(self.now);
        self.redo.clear();
        self.now = next;
        self.refresh_crop(base);
    }

    /// Enter: the picture is cut to the rectangle, unless the rectangle is the whole of it.
    fn apply(&mut self, base: PixelSize) {
        let Cropping::On(crop) = &self.crop else {
            return;
        };
        let shown = self.now.turned_size(base);
        let draft = crop.draft;
        self.crop = Cropping::Off;
        if draft != CropBox::whole(shown) {
            let next = self.now.cropped(base, draft.rect());
            self.change(base, next);
        }
    }

    /// A rectangle drawn over the picture as it is now, with nothing cut yet.
    fn restart_crop(&mut self, base: PixelSize, aspect: CropAspect) {
        let shown = self.now.turned_size(base);
        let whole = CropBox::whole(shown);
        let draft = match aspect.ratio(shown) {
            Some(ratio) => whole.shaped(ratio),
            None => whole,
        };
        self.crop = Cropping::On(Crop {
            draft,
            aspect,
            held: None,
        });
    }

    /// The picture changed under a rectangle that is being drawn: it starts again over the new
    /// picture, held to the same proportions.
    fn refresh_crop(&mut self, base: PixelSize) {
        if let Cropping::On(crop) = &self.crop {
            let aspect = crop.aspect;
            self.restart_crop(base, aspect);
        }
    }
}
