//! A decoded picture on the GPU, and the back end that puts it there. Decoding and the upload run
//! on a worker: the picture goes from the decoder into the window's `TextureHandle` without
//! touching the UI thread, and the document the UI receives is only the picture's size.

use crate::io::{Backend, OpenError, OpenLink, Stop};
use crate::{FrameDelays, FrameIndex, Runs, Ticket};
use anyview_core::{
    ByteLen, FactLabel, FactValue, Facts, FormatDetail, FormatKind, Peek, PeekBudget, PixelArea,
    PixelSize, RasterFormat, Sniffed, Source,
};
use anyview_image::{
    Animation, Decoded, ImagePeek, Plays, RasterPeek, Rgba8, VectorPeek, declared_size, decode,
};
use ds_blitz::{PixelFormat, Pixels, TextureHandle};
use std::sync::Arc;
use std::time::Duration;

/// What a first frame may spend: a picture about the size of a window, from a file read whole.
/// The time is the pool's to enforce; the decoder reads no clock.
const FIRST_FRAME: PeekBudget = PeekBudget {
    bytes: ByteLen(64 * 1024 * 1024),
    pixels: PixelArea(4 * 1024 * 1024),
    time: Duration::from_secs(1),
};

/// One picture of an animation: where its pixels are and how long it stays.
#[derive(Debug, Clone)]
pub(crate) struct StripFrame {
    pub texture: TextureHandle,
    pub delay: Duration,
}

/// The frames of an animation, each in its own texture so that showing the next one is choosing
/// a texture, with no pixels moved on the UI thread.
#[derive(Debug)]
pub(crate) struct FrameStrip {
    frames: Vec<StripFrame>,
}

/// An opened picture. The pixels are in `texture`; an animation also has its frames in `strip`.
#[derive(Debug, Clone)]
pub struct RasterDoc {
    /// Where the pixels are, upright: the picture, or the first frame of an animation.
    pub texture: TextureHandle,
    /// The picture's size in pixels.
    pub size: PixelSize,
    /// The size of what `texture` holds: smaller than `size` for a first frame shown while the
    /// picture decodes, and drawn to fill the same box.
    pub held: PixelSize,
    /// How many pictures the file holds.
    pub frames: u32,
    /// The rows of the Info tab.
    pub facts: Facts,
    pub(crate) strip: Option<Arc<FrameStrip>>,
    /// How many runs the animation asks for.
    pub(crate) runs: Runs,
}

impl RasterDoc {
    /// The texture that shows `frame`: the picture itself when the file is not an animation.
    pub(crate) fn texture_at(&self, frame: FrameIndex) -> &TextureHandle {
        self.strip
            .as_ref()
            .and_then(|strip| strip.frames.get(frame.0 as usize))
            .map_or(&self.texture, |shown| &shown.texture)
    }

    /// How long each frame of an animation stays on screen; none for a still.
    pub(crate) fn delays(&self) -> FrameDelays {
        FrameDelays(self.strip.as_ref().map_or_else(Default::default, |strip| {
            strip.frames.iter().map(|shown| shown.delay).collect()
        }))
    }

    /// Whether the file is an animation with all its frames ready.
    pub(crate) fn plays(&self) -> bool {
        self.strip.is_some()
    }
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
    match decode(&target.source, &target.sniffed)? {
        Decoded::Still(picture) => {
            upload(&target.texture, &picture)?;
            Ok(doc_of(target, picture.size(), 1, None, Runs::Forever))
        }
        Decoded::HeldStill { picture, frames } => {
            upload(&target.texture, &picture)?;
            let mut doc = doc_of(target, picture.size(), frames.0, None, Runs::Forever);
            doc.facts = doc.facts.with(
                FactLabel::Frames,
                FactValue::text(format!(
                    "{} (too large to play; showing the first)",
                    frames.0
                )),
            );
            Ok(doc)
        }
        Decoded::Animated(animation) => {
            let strip = upload_frames(&target.texture, &animation)?;
            let first = animation.frames.first().pixels.size();
            let count = u32::try_from(animation.frames.count().get()).unwrap_or(u32::MAX);
            let runs = match animation.plays {
                Plays::Forever => Runs::Forever,
                Plays::Times(times) => Runs::Times(times),
            };
            Ok(doc_of(target, first, count, Some(Arc::new(strip)), runs))
        }
    }
}

fn doc_of(
    target: &RasterTarget,
    size: PixelSize,
    frames: u32,
    strip: Option<Arc<FrameStrip>>,
    runs: Runs,
) -> RasterDoc {
    RasterDoc {
        texture: target.texture.clone(),
        size,
        held: size,
        frames,
        facts: facts(&target.source, &target.sniffed, size, frames),
        strip,
        runs,
    }
}

/// Every frame of `animation` in a texture of its own: the first goes into `first`, the others
/// into new handles on the same GPU.
fn upload_frames(first: &TextureHandle, animation: &Animation) -> Result<FrameStrip, OpenError> {
    let mut frames = Vec::new();
    for (index, frame) in animation.frames.iter().enumerate() {
        let texture = if index == 0 {
            first.clone()
        } else {
            first.gpu().handle()
        };
        upload(&texture, &frame.pixels)?;
        frames.push(StripFrame {
            texture,
            delay: Duration::from_micros(frame.delay.0),
        });
    }
    Ok(FrameStrip { frames })
}

/// The first frame of a picture, cheaper than decoding it whole: the host's small picture of the
/// file when it has one (given the size the file declares, so the box it fills is the final one),
/// otherwise the first frame of an animation or a drawn-small vector image. `None` for a picture
/// that would take as long to peek as to open.
pub(crate) fn first_frame(
    src: &Source,
    sniffed: &Sniffed,
    link: &OpenLink,
) -> Result<Option<RasterDoc>, OpenError> {
    let Some((picture, size, frames, facts)) = cheap_picture(src, sniffed, link)? else {
        return Ok(None);
    };
    upload(&link.texture, &picture)?;
    Ok(Some(RasterDoc {
        texture: link.texture.clone(),
        size,
        held: picture.size(),
        frames,
        facts,
        strip: None,
        runs: Runs::Forever,
    }))
}

type Cheap = (Rgba8, PixelSize, u32, Facts);

fn cheap_picture(
    src: &Source,
    sniffed: &Sniffed,
    link: &OpenLink,
) -> Result<Option<Cheap>, OpenError> {
    if let Some(picture) = link.first_frames.picture(src) {
        let size = declared_size(src, sniffed)?.unwrap_or_else(|| picture.size());
        let facts = facts(src, sniffed, size, 1);
        return Ok(Some((picture, size, 1, facts)));
    }
    let (peeked, facts): (ImagePeek, Facts) = match (sniffed.kind(), sniffed.detail()) {
        (FormatKind::Vector, _) => {
            let peeked = VectorPeek::peek(src, sniffed, &FIRST_FRAME)?;
            let facts = VectorPeek::facts(&peeked);
            (peeked, facts)
        }
        (FormatKind::Raster, FormatDetail::Raster(RasterFormat::Gif | RasterFormat::Webp)) => {
            let peeked = RasterPeek::peek(src, sniffed, &FIRST_FRAME)?;
            let facts = RasterPeek::facts(&peeked);
            (peeked, facts)
        }
        _ => return Ok(None),
    };
    let frames = peeked.frames.0;
    Ok(Some((peeked.picture, peeked.source_size, frames, facts)))
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
