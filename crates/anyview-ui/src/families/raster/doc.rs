//! A decoded picture on the GPU, and the back end that puts it there. Decoding and the upload run
//! on a worker: the picture goes from the decoder into the window's `TextureHandle` without
//! touching the UI thread, and the document the UI receives is only the picture's size.

use crate::io::{Backend, ImagePlugins, Need, OpenError, OpenLink, PluginPicture, Stop};
use crate::{EditCaution, EditOffer, FrameDelays, FrameIndex, Runs, Ticket};
use anyview_core::{
    ByteLen, FactLabel, FactValue, Facts, FormatDetail, FormatKind, Helper, Input, Peek,
    PeekBudget, PixelArea, PixelLen, PixelSize, RasterFormat, Resize, Sniffed, Source,
};
use anyview_image::{
    Animation, Decoded, Fidelity, ImageError, ImagePeek, Plays, RasterPeek, Rgba8, VectorPeek,
    declared_size, decode_bytes, fidelity, file_bytes, picture_facts, resized,
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

/// The most pixels a plugin may be asked to decode: a 64 megapixel photograph, 256 MiB of RGBA.
const PLUGIN_AREA: PixelArea = PixelArea(64 * 1024 * 1024);

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
    /// Set when there is no picture to show because the plugin that decodes this kind of file is
    /// not installed: the stage shows the facts and this row instead of the texture.
    pub needs: Option<Need>,
    /// The tool the system lacks that would show this file fully (a raw file shown from its
    /// preview, or a card with no picture): installing it opens the file again.
    pub lacking: Option<Helper>,
    /// What saving a turn or flip of the file costs.
    pub offer: EditOffer,
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
    /// The plugins that decode what the viewer cannot.
    pub plugins: Arc<dyn ImagePlugins>,
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
    // The file is read once: the decoder, the facts and the edit offer share these bytes.
    let bytes = file_bytes(&Input::from(&target.source))?;
    let mut doc = decoded(target, &bytes)?;
    doc.offer = offer_of(&target.source, &target.sniffed, &bytes);
    Ok(doc)
}

/// What saving a turn or flip of the file costs, read from its headers. A file too large to read
/// whole for this is asked about nothing: the edit's own limits answer.
fn offer_of(source: &Source, sniffed: &Sniffed, bytes: &[u8]) -> EditOffer {
    const LIMIT: u64 = 512 * 1024 * 1024;
    if source.stamp().len.0 > LIMIT {
        return EditOffer::Plain;
    }
    match fidelity(bytes, sniffed) {
        Fidelity::Intact => EditOffer::Plain,
        Fidelity::Loses(loss) => EditOffer::Asks(EditCaution::Loses(loss.sentence())),
        Fidelity::Impossible => EditOffer::Withheld,
    }
}

fn decoded(target: &RasterTarget, bytes: &[u8]) -> Result<RasterDoc, OpenError> {
    if matches!(
        target.sniffed.detail(),
        FormatDetail::Raster(RasterFormat::Raw)
    ) {
        return raw_into(target, bytes);
    }
    match decode_bytes(bytes, &target.sniffed) {
        Ok(Decoded::Still(picture)) => {
            let held = upload(&target.texture, &picture)?;
            Ok(holding(
                doc_of(target, bytes, picture.size(), 1, None, Runs::Forever),
                held,
            ))
        }
        Ok(Decoded::HeldStill { picture, frames }) => {
            let held = upload(&target.texture, &picture)?;
            let mut doc = holding(
                doc_of(target, bytes, picture.size(), frames.0, None, Runs::Forever),
                held,
            );
            doc.facts = doc.facts.with(
                FactLabel::Frames,
                FactValue::text(format!(
                    "{} (too large to play; showing the first)",
                    frames.0
                )),
            );
            Ok(doc)
        }
        Ok(Decoded::Animated(animation)) => {
            let (strip, held) = upload_frames(&target.texture, &animation)?;
            let first = animation.frames.first().pixels.size();
            let count = u32::try_from(animation.frames.count().get()).unwrap_or(u32::MAX);
            let runs = match animation.plays {
                Plays::Forever => Runs::Forever,
                Plays::Times(times) => Runs::Times(times),
            };
            Ok(holding(
                doc_of(target, bytes, first, count, Some(Arc::new(strip)), runs),
                held,
            ))
        }
        // HEIC has no decoder here, and AVIF has one only in a build that kept it: a plugin may.
        Err(
            error @ (ImageError::Unsupported {
                format: RasterFormat::Heic,
            }
            | ImageError::NotCompiledIn {
                format: RasterFormat::Avif,
            }),
        ) => plugin_into(target, bytes, error),
        Err(error) => Err(error.into()),
    }
}

/// A picture only a plugin can decode: its pixels, or the facts and the row naming the package.
/// `unserved` is what the viewer says when no plugin is installed and none is known.
fn plugin_into(
    target: &RasterTarget,
    bytes: &[u8],
    unserved: ImageError,
) -> Result<RasterDoc, OpenError> {
    match target
        .plugins
        .decode(&target.source, &target.sniffed, PLUGIN_AREA)
    {
        PluginPicture::Pixels(picture) => {
            let held = upload(&target.texture, &picture)?;
            Ok(holding(
                doc_of(target, bytes, picture.size(), 1, None, Runs::Forever),
                held,
            ))
        }
        PluginPicture::Missing(needs) => Ok(blank_doc(target, bytes, needs)),
        PluginPicture::Unserved => Err(unserved.into()),
        PluginPicture::Failed(reason) => Err(OpenError::Plugin(reason)),
    }
}

/// A camera raw file: the plugin's full development when one is installed and works; else the
/// preview inside the file, with a row offering the plugin for the full picture; else, for a file
/// with no readable preview, the facts and that row.
fn raw_into(target: &RasterTarget, bytes: &[u8]) -> Result<RasterDoc, OpenError> {
    let developed = target
        .plugins
        .decode(&target.source, &target.sniffed, PLUGIN_AREA);
    let needs = match developed {
        PluginPicture::Pixels(picture) => {
            let held = upload(&target.texture, &picture)?;
            return Ok(holding(
                doc_of(target, bytes, picture.size(), 1, None, Runs::Forever),
                held,
            ));
        }
        PluginPicture::Missing(needs) => Some(needs),
        PluginPicture::Failed(reason) => {
            eprintln!("anyview: the raw plugin could not develop the file: {reason}");
            None
        }
        PluginPicture::Unserved => None,
    };
    match decode_bytes(bytes, &target.sniffed) {
        Ok(Decoded::Still(picture)) => {
            let held = upload(&target.texture, &picture)?;
            let mut doc = holding(
                doc_of(target, bytes, picture.size(), 1, None, Runs::Forever),
                held,
            );
            if let Some(needs) = needs {
                doc.facts = doc.facts.with(needs.fact.label, needs.fact.value);
                doc.lacking = needs.helper;
            }
            Ok(doc)
        }
        Ok(Decoded::Animated(_) | Decoded::HeldStill { .. }) => Err(OpenError::Unrecognised),
        Err(ImageError::NoPreview) => match needs {
            Some(needs) => Ok(blank_doc(target, bytes, needs)),
            None => Err(ImageError::NoPreview.into()),
        },
        Err(error) => Err(error.into()),
    }
}

/// A document with no picture: the facts of the file and the row naming what would show it.
fn blank_doc(target: &RasterTarget, bytes: &[u8], needs: Need) -> RasterDoc {
    let one = PixelSize {
        width: PixelLen(1),
        height: PixelLen(1),
    };
    let mut doc = doc_of(target, bytes, one, 1, None, Runs::Forever);
    doc.facts = Facts::empty()
        .with(
            FactLabel::Kind,
            FactValue::text(kind_words(&target.sniffed)),
        )
        .with(FactLabel::Size, FactValue::size(target.source.stamp().len))
        .with(needs.fact.label, needs.fact.value.clone());
    doc.lacking = needs.helper;
    doc.needs = Some(needs);
    doc
}

/// `doc` for a texture that holds the picture at `held`, which is smaller than the picture when
/// the GPU cannot take it whole.
fn holding(doc: RasterDoc, held: PixelSize) -> RasterDoc {
    RasterDoc { held, ..doc }
}

fn doc_of(
    target: &RasterTarget,
    bytes: &[u8],
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
        facts: facts(size, frames).then(metadata_facts(bytes, &target.sniffed)),
        needs: None,
        lacking: None,
        offer: EditOffer::Plain,
        strip,
        runs,
    }
}

/// Every frame of `animation` in a texture of its own: the first goes into `first`, the others
/// into new handles on the same GPU.
fn upload_frames(
    first: &TextureHandle,
    animation: &Animation,
) -> Result<(FrameStrip, PixelSize), OpenError> {
    let mut frames = Vec::new();
    let mut held = animation.frames.first().pixels.size();
    for (index, frame) in animation.frames.iter().enumerate() {
        let texture = if index == 0 {
            first.clone()
        } else {
            first.gpu().handle()
        };
        held = upload(&texture, &frame.pixels)?;
        frames.push(StripFrame {
            texture,
            delay: Duration::from_micros(frame.delay.0),
        });
    }
    Ok((FrameStrip { frames }, held))
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
    let held = upload(&link.texture, &picture)?;
    Ok(Some(RasterDoc {
        texture: link.texture.clone(),
        size,
        held,
        frames,
        facts,
        needs: None,
        lacking: None,
        offer: EditOffer::Plain,
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
        let facts = facts(size, 1).then(file_metadata_facts(src, sniffed));
        return Ok(Some((picture, size, 1, facts)));
    }
    let (peeked, facts): (ImagePeek, Facts) = match (sniffed.kind(), sniffed.detail()) {
        (FormatKind::Vector, _) => {
            let peeked = VectorPeek::peek(&Input::from(src), sniffed, &FIRST_FRAME)?;
            let facts = VectorPeek::facts(&peeked);
            (peeked, facts)
        }
        // A raw file's embedded preview is a cheap first frame while a plugin develops the whole.
        (
            FormatKind::Raster,
            FormatDetail::Raster(RasterFormat::Gif | RasterFormat::Webp | RasterFormat::Raw),
        ) => {
            let peeked = RasterPeek::peek(&Input::from(src), sniffed, &FIRST_FRAME)?;
            let facts = RasterPeek::facts(&peeked);
            (peeked, facts)
        }
        _ => return Ok(None),
    };
    let frames = peeked.frames.0;
    Ok(Some((peeked.picture, peeked.source_size, frames, facts)))
}

/// Puts `picture` in `texture` and says what size it holds. A picture longer on a side than the
/// GPU's texture limit goes up shrunk to fit it, as a viewer shows a huge image; the doc keeps
/// the picture's own size, so the zoom and the facts stay true.
fn upload(texture: &TextureHandle, picture: &Rgba8) -> Result<PixelSize, OpenError> {
    let limit = texture
        .gpu()
        .device()
        .map_or(u32::MAX, |device| device.limits().max_texture_dimension_2d);
    let size = picture.size();
    if size.width.0.max(size.height.0) > limit {
        return upload(
            texture,
            &resized(picture, Resize::LongEdge(PixelLen(limit))),
        );
    }
    let pixels = Pixels::new(
        PixelFormat::Rgba8Straight,
        size.width.0,
        size.height.0,
        picture.bytes(),
    )
    .map_err(|_| OpenError::Unrecognised)?;
    texture.update(&pixels).map_err(OpenError::Gpu)?;
    Ok(size)
}

/// The rows of the Info tab that need no file: the picture's size and, for an animation, its
/// frames. The General section (kind, size, dates) is the window's.
fn facts(size: PixelSize, frames: u32) -> Facts {
    let facts = Facts::empty().with(FactLabel::Dimensions, FactValue::dimensions(size));
    if frames > 1 {
        facts.with(FactLabel::Frames, FactValue::text(frames.to_string()))
    } else {
        facts
    }
}

/// Colour, density, camera and location, read from the file's own headers in `bytes`. An SVG has
/// none.
fn metadata_facts(bytes: &[u8], sniffed: &Sniffed) -> Facts {
    if sniffed.kind() == FormatKind::Raster {
        picture_facts(bytes, sniffed)
    } else {
        Facts::empty()
    }
}

/// How much of a file the cheap first look reads for what it says about itself: the EXIF block of
/// a JPEG, PNG, WebP or HEIC sits in the first blocks, and a TIFF's in most files.
const FIRST_LOOK_METADATA_READ: u64 = 32 * 1024 * 1024;

/// [`metadata_facts`] of the start of the file at `source`, for the first look that has not read
/// the file whole. A file that cannot be read has none.
fn file_metadata_facts(source: &Source, sniffed: &Sniffed) -> Facts {
    use std::io::Read;
    let mut bytes = Vec::new();
    let read = std::fs::File::open(source.path().as_path())
        .and_then(|file| file.take(FIRST_LOOK_METADATA_READ).read_to_end(&mut bytes));
    match read {
        Ok(_) => metadata_facts(&bytes, sniffed),
        Err(_) => Facts::empty(),
    }
}

/// The words for what a file is: its media type.
pub(crate) fn kind_words(sniffed: &Sniffed) -> String {
    sniffed.mime().as_str().to_owned()
}
