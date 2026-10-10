//! The picture being edited: what has been done to it, what can be taken back or done again, and
//! the crop rectangle while it is being drawn. Nothing here touches the file: the edits wait in
//! this state until the person saves.

use super::crop::{CropAspect, CropBox, CropGrip};
use anyview_core::{Adjust, Axis, DocPoint, DocUnit, PixelSize, QuarterTurn};

/// Whether the open picture can be saved with changes: its format has a writer and the file
/// takes a save in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PictureEditing {
    /// Tools that change the picture are offered.
    Allowed,
    /// The picture can be looked at and no more.
    #[default]
    Locked,
}

/// A crop being drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Crop {
    /// The rectangle now.
    pub(super) draft: CropBox,
    /// The proportions it is held to.
    pub(super) aspect: CropAspect,
    /// The grip a drag has hold of.
    pub(super) held: Option<Held>,
}

/// A grip taken hold of: the rectangle as it was at the press, and where the pointer was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Held {
    pub(super) grip: CropGrip,
    pub(super) from: CropBox,
    pub(super) at: DocPoint,
}

/// Whether a crop is being drawn.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) enum Cropping {
    #[default]
    Off,
    On(Crop),
}

/// The open picture's edits. They are about the picture as it was opened, so a new picture
/// (`Landed`) starts them afresh.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PictureEdits {
    pub(super) base: Option<PixelSize>,
    pub(super) editing: PictureEditing,
    pub(super) now: Adjust,
    pub(super) undo: Vec<Adjust>,
    pub(super) redo: Vec<Adjust>,
    pub(super) crop: Cropping,
}

/// What changes the picture's edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PictureEditIn {
    /// A picture is on screen (or the file was opened again after a save): its size, and whether
    /// it can be changed. Whatever was done to the last one is gone.
    Landed {
        /// The picture's size in pixels, when it has been decoded.
        size: Option<PixelSize>,
        /// Whether changes are on offer.
        editing: PictureEditing,
    },
    /// Turn the picture as shown.
    Turn(QuarterTurn),
    /// Mirror the picture as shown.
    Flip(Axis),
    /// Scale the picture as shown to this size.
    Resize(PixelSize),
    /// The crop tool was chosen: the whole picture is the rectangle.
    CropOn,
    /// Another tool was chosen: the rectangle goes.
    CropOff,
    /// A press on the picture at this point (the picture as shown), with `reach` the distance from
    /// an edge of the rectangle that still takes hold of it.
    Grab {
        /// Where the pointer is.
        at: DocPoint,
        /// How near an edge counts as on it.
        reach: DocUnit,
    },
    /// The pointer moved while a grip is held.
    Drag(DocPoint),
    /// The pointer was let go.
    Release,
    /// Hold the rectangle to these proportions.
    Hold(CropAspect),
    /// Enter: cut the picture to the rectangle.
    Apply,
    /// Let every edit go: the picture is as it was opened.
    Discard,
    /// Take back the last edit.
    Undo,
    /// Do again the edit that was taken back.
    Redo,
}

impl PictureEdits {
    /// What has been done to the picture.
    pub fn adjust(&self) -> Adjust {
        self.now
    }

    /// Whether anything has been done that is not saved.
    pub fn is_edited(&self) -> bool {
        !self.now.is_none()
    }

    /// Whether the picture can be changed: it is decoded and its file takes a save.
    pub fn can_edit(&self) -> bool {
        self.base.is_some() && self.editing == PictureEditing::Allowed
    }

    /// The picture's size as it was opened.
    pub fn base(&self) -> Option<PixelSize> {
        self.base
    }

    /// The picture's size as it is shown now, before a resize: what a crop is drawn on.
    pub fn shown_size(&self) -> Option<PixelSize> {
        self.base.map(|base| self.now.turned_size(base))
    }

    /// The size the saved picture will have.
    pub fn result_size(&self) -> Option<PixelSize> {
        self.base.map(|base| self.now.result_size(base))
    }

    /// Whether the crop tool is up.
    pub fn is_cropping(&self) -> bool {
        matches!(self.crop, Cropping::On(_))
    }

    /// The crop rectangle, while the tool is up.
    pub fn draft(&self) -> Option<CropBox> {
        match &self.crop {
            Cropping::On(crop) => Some(crop.draft),
            Cropping::Off => None,
        }
    }

    /// The proportions the rectangle is held to.
    pub fn aspect(&self) -> CropAspect {
        match &self.crop {
            Cropping::On(crop) => crop.aspect,
            Cropping::Off => CropAspect::default(),
        }
    }

    /// Whether a grip of the rectangle is held.
    pub fn is_dragging(&self) -> bool {
        matches!(&self.crop, Cropping::On(crop) if crop.held.is_some())
    }

    /// Whether there is an edit to take back.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether there is an edit to do again.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}
