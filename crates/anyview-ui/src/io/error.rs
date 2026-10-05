//! Why opening a file failed, in the terms a caller acts on.

use crate::LoadFailure;
use crate::families::PdfFailure;
use ds_blitz::GpuError;
use std::io::ErrorKind;

/// Why a file could not be probed or opened.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum OpenError {
    /// The file system refused.
    #[error("cannot read the file: {0}")]
    Read(ErrorKind),
    /// The image decoder refused.
    #[error(transparent)]
    Image(#[from] anyview_image::ImageError),
    /// The text reader refused.
    #[error(transparent)]
    Text(#[from] anyview_text::TextError),
    /// The PDF reader refused.
    #[error(transparent)]
    Pdf(#[from] PdfFailure),
    /// The picture cannot be put on the GPU.
    #[error("the picture cannot be shown: {0}")]
    Gpu(GpuError),
    /// The player could not be started.
    #[error("the player could not start: {0}")]
    Media(String),
    /// A plugin that serves this file could not make its picture.
    #[error("the plugin could not show the picture: {0}")]
    Plugin(String),
    /// The probe could not tell what the file is (a zip needs its entries listed).
    #[error("the viewer cannot tell what this file is")]
    Unrecognised,
}

impl OpenError {
    /// What the load machine is told.
    pub fn failure(&self) -> LoadFailure {
        match self {
            OpenError::Read(ErrorKind::NotFound) => LoadFailure::NotFound,
            OpenError::Read(_) => LoadFailure::Unreadable,
            OpenError::Image(error) => image_failure(error),
            OpenError::Text(anyview_text::TextError::Read { kind, .. }) => {
                OpenError::Read(*kind).failure()
            }
            OpenError::Text(_) => LoadFailure::Damaged,
            OpenError::Pdf(failure) => pdf_failure(*failure),
            OpenError::Gpu(_)
            | OpenError::Media(_)
            | OpenError::Plugin(_)
            | OpenError::Unrecognised => LoadFailure::Unsupported,
        }
    }
}

fn pdf_failure(failure: PdfFailure) -> LoadFailure {
    match failure {
        PdfFailure::Unreadable(kind) => OpenError::Read(kind).failure(),
        PdfFailure::Locked => LoadFailure::Unsupported,
        PdfFailure::Empty | PdfFailure::Damaged => LoadFailure::Damaged,
    }
}

fn image_failure(error: &anyview_image::ImageError) -> LoadFailure {
    use anyview_image::ImageError as E;
    match error {
        E::Read { kind, .. } => OpenError::Read(*kind).failure(),
        E::WrongKind { .. }
        | E::Unsupported { .. }
        | E::NotCompiledIn { .. }
        | E::NoPreview
        | E::TooLarge { .. }
        | E::NotSavable { .. }
        | E::NotSavableAnimated
        | E::NotAnImageEdit { .. } => LoadFailure::Unsupported,
        E::Decode { .. }
        | E::NoBudget
        | E::PixelsMismatch { .. }
        | E::Encode { .. }
        | E::Container { .. }
        | E::Exif { .. } => LoadFailure::Damaged,
    }
}
