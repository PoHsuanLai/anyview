//! The one way into the peek: [`look`] a file, as [`Peeking`] says.

use crate::any::{AnyPeeked, failed_card, peek_looking};
use crate::frames::{NoStills, StillSource};
use crate::probe::probe;
use crate::unavailable::Unavailable;
use anyview_core::{ByteLen, Input, PeekBudget, PixelArea, PixelLen, PixelSize};
use std::sync::Arc;
use std::time::Duration;

/// The box, in pixels, that the pane fits a page or a picture into: a 360 px pane's width inside
/// its 16 px padding by the height it gives its media. The pane pins it equal to quire's
/// `PANE_MEDIA`.
pub(crate) const PANE_FIT: PixelSize = PixelSize {
    width: PixelLen(328),
    height: PixelLen(220),
};

/// What a look at a file is asked to do: what it may spend, the box a page is drawn in, and where
/// a picture of a video comes from. It is `#[non_exhaustive]`, so a setting added later breaks no
/// caller: start from [`Peeking::pane`] and change one setting with the `with_*` methods.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Peeking {
    /// What a peek may spend: bytes read, pixels decoded, time.
    pub budget: PeekBudget,
    /// The box a PDF's first page is rasterised to fit, in pixels at one to one (a budget that
    /// allows it draws the page twice as sharp).
    pub fit: PixelSize,
    /// The host's pictures of files this crate does not decode. By default there are none.
    pub stills: Arc<dyn StillSource>,
}

impl Peeking {
    /// What the launcher's pane asks: it reads a PDF whole (up to 64 MiB), fits a picture to a
    /// pane's worth of pixels (a million), takes half a second, and draws a page into 328 by 220
    /// px.
    #[must_use]
    pub fn pane() -> Self {
        Peeking {
            budget: PeekBudget {
                bytes: ByteLen(64 * 1024 * 1024),
                pixels: PixelArea(1_000_000),
                time: Duration::from_millis(500),
            },
            fit: PANE_FIT,
            stills: Arc::new(NoStills),
        }
    }

    /// This look spending no more than `budget`.
    #[must_use]
    pub fn with_budget(self, budget: PeekBudget) -> Self {
        Peeking { budget, ..self }
    }

    /// This look drawing a page to fit `fit`.
    #[must_use]
    pub fn with_fit(self, fit: PixelSize) -> Self {
        Peeking { fit, ..self }
    }

    /// This look taking a video's picture from `stills` when the file carries none.
    #[must_use]
    pub fn with_stills(self, stills: Arc<dyn StillSource>) -> Self {
        Peeking { stills, ..self }
    }
}

impl Default for Peeking {
    fn default() -> Self {
        Peeking::pane()
    }
}

/// Looks at a file: what it is, and the card a pane draws for it. `src` is a path made with
/// `anyview_fs::OnDisk::on_disk`, or any bytes a host injects (`Input::from((name, bytes))`).
/// Blocking, so run it on a worker. It never fails: a file that cannot be read or peeked comes
/// back as [`Body::Unavailable`](crate::Body::Unavailable) with the reason, and the facts the file
/// can still give.
///
/// ```no_run
/// use anyview_core::FilePath;
/// use anyview_peek::{Peeking, look};
/// use anyview_fs::OnDisk;
///
/// # fn main() -> Result<(), anyview_core::CoreError> {
/// let path = FilePath::new("/home/ann/photo.jpg")?;
/// let card = look(path.on_disk(), &Peeking::pane());
/// println!("{}: {} facts", card.name, card.facts.rows().len());
/// # Ok(())
/// # }
/// ```
#[must_use]
pub fn look(src: impl Into<Input>, peeking: &Peeking) -> AnyPeeked {
    let input = src.into();
    match probe(&input) {
        Ok(probed) => peek_looking(&probed.input, &probed.sniffed, peeking),
        Err(error) => failed_card(&input, Unavailable::of(&error)),
    }
}
