//! A decoded picture on the GPU, and the back end that puts it there. Decoding and the upload run
//! on a worker: the picture goes from the decoder into the window's `TextureHandle` without
//! touching the UI thread, and the document the UI receives is only the picture's size.

use crate::Ticket;
use crate::io::{Backend, OpenError, Stop};
use anyview_core::{FactLabel, FactValue, Facts, PixelSize, Sniffed, Source};
use anyview_image::{Decoded, Rgba8, decode};
use ds_blitz::{PixelFormat, Pixels, TextureHandle};

/// An opened picture. The pixels are in `texture`; `frames` says whether it was animated (only
/// its first frame is shown).
#[derive(Debug, Clone)]
pub struct RasterDoc {
    /// Where the pixels are, upright.
    pub texture: TextureHandle,
    /// The picture's size in pixels.
    pub size: PixelSize,
    /// How many pictures the file holds.
    pub frames: u32,
    /// The rows of the Info tab.
    pub facts: Facts,
}

/// What to open and where to put it.
#[derive(Debug, Clone)]
pub struct RasterTarget {
    /// The file.
    pub source: Source,
    /// How it was sniffed.
    pub sniffed: Sniffed,
    /// The window's texture for it.
    pub texture: TextureHandle,
}

/// The one job of the back end.
#[derive(Debug, Clone, Copy)]
pub enum RasterJob {
    /// Decode the file and upload its first picture.
    Decode { ticket: Ticket },
}

/// What a decode made.
#[derive(Debug)]
pub struct RasterDone {
    /// The load it belongs to.
    pub ticket: Ticket,
    /// The picture, or why not.
    pub result: Result<RasterDoc, OpenError>,
}

/// The image back end: decoding is one blocking call that cannot be interrupted partway, so a
/// stale decode finishes and its result is dropped by the ticket.
#[derive(Debug, Clone, Copy)]
pub struct RasterBackend;

impl Backend for RasterBackend {
    type Doc = RasterTarget;
    type Worker = ();
    type Job = RasterJob;
    type Done = RasterDone;

    fn run(doc: &RasterTarget, _worker: &mut (), job: RasterJob, _stop: &Stop) -> RasterDone {
        match job {
            RasterJob::Decode { ticket } => RasterDone {
                ticket,
                result: decode_into(doc),
            },
        }
    }
}

fn decode_into(target: &RasterTarget) -> Result<RasterDoc, OpenError> {
    let (first, frames) = match decode(&target.source, &target.sniffed)? {
        Decoded::Still(picture) => (picture, 1),
        Decoded::Animated(animation) => {
            let count = u32::try_from(animation.frames.count().get()).unwrap_or(u32::MAX);
            (animation.frames.first().pixels.clone(), count)
        }
    };
    upload(&target.texture, &first)?;
    let size = first.size();
    Ok(RasterDoc {
        texture: target.texture.clone(),
        size,
        frames,
        facts: facts(&target.source, &target.sniffed, size, frames),
    })
}

fn upload(texture: &TextureHandle, picture: &Rgba8) -> Result<(), OpenError> {
    let size = picture.size();
    let pixels = Pixels::new(
        PixelFormat::Rgba8Straight,
        size.width.0,
        size.height.0,
        picture.bytes(),
    )
    .map_err(|_| OpenError::Unrecognised)?;
    texture.update(&pixels).map_err(OpenError::Gpu)
}

fn facts(source: &Source, sniffed: &Sniffed, size: PixelSize, frames: u32) -> Facts {
    let facts = Facts::empty()
        .with(FactLabel::Kind, FactValue::text(kind_words(sniffed)))
        .with(FactLabel::Dimensions, FactValue::dimensions(size))
        .with(FactLabel::Size, FactValue::size(source.stamp().len));
    if frames > 1 {
        facts.with(FactLabel::Frames, FactValue::text(frames.to_string()))
    } else {
        facts
    }
}

/// The words for what a file is: its media type.
pub(crate) fn kind_words(sniffed: &Sniffed) -> String {
    sniffed.mime().as_str().to_owned()
}
