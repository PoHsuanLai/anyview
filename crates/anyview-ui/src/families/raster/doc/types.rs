//! A decoded picture on the GPU: the document, what opens it, and the back end that puts it there.

use super::decode::decode_into;
use crate::io::{Backend, ImagePlugins, Need, OpenError, Stop};
use crate::{EditOffer, FrameDelays, FrameIndex, Runs, Ticket};
use anyview_core::{Facts, Helper, PixelSize, Sniffed, Source};
use ds_blitz::TextureHandle;
use std::sync::Arc;
use std::time::Duration;

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
    pub(super) frames: Vec<StripFrame>,
}
/// An opened picture. The pixels are in `texture`; an animation also has its frames in `strip`.
#[derive(Debug, Clone)]
pub struct RasterDoc {
    /// What the file is called.
    pub name: String,
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
pub struct RasterOpen {
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
    Decode {
        /// The load this work belongs to.
        ticket: Ticket,
    },
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
    type Doc = RasterOpen;
    type Worker = ();
    type Job = RasterJob;
    type Done = RasterDone;

    fn run(doc: &RasterOpen, _worker: &mut (), job: RasterJob, _stop: &Stop) -> RasterDone {
        match job {
            RasterJob::Decode { ticket } => RasterDone {
                ticket,
                result: decode_into(doc),
            },
        }
    }
}
