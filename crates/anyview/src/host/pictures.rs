//! The first frame of a picture from the thumbnail cache the desktop shares: the small picture
//! some program made of the file earlier, shown while the file decodes.

use crate::media::MediaPlugins;
use anyview_core::{Input, Sniffed};
use anyview_image::Rgba8;
use anyview_peek::StillSource;
use anyview_platform::{ThumbSize, ThumbnailCache};
use std::sync::Arc;

/// The sizes tried, the larger first: it is shown big and scaled down by nothing.
const SIZES: [ThumbSize; 2] = [ThumbSize::Large, ThumbSize::Normal];

/// The cache `C` as the views' source of first frames.
#[derive(Debug)]
pub struct CachedPictures<C>(pub C);

impl<C: ThumbnailCache + std::fmt::Debug + Send + Sync + 'static> StillSource
    for CachedPictures<C>
{
    fn still(&self, source: &Input) -> Option<Rgba8> {
        let path = source.path()?;
        let stamp = source.stamp();
        SIZES.iter().find_map(|size| {
            let found = self.0.lookup(path, &stamp, *size).ok().flatten()?;
            Rgba8::new(found.size(), found.rgba().to_vec()).ok()
        })
    }
}

/// The pictures of `S` and, for a video it has no thumbnail of, a frame the installed plugins
/// decode (the FFmpeg plugin takes one a little way in): what a pane's card shows in place of a
/// plate.
#[derive(Debug)]
pub struct PosterPictures<S> {
    cached: S,
    plugins: Arc<MediaPlugins>,
}

impl<S> PosterPictures<S> {
    /// `cached`'s pictures, with frames from `plugins` behind them.
    pub fn new(cached: S, plugins: Arc<MediaPlugins>) -> PosterPictures<S> {
        PosterPictures { cached, plugins }
    }
}

impl<S: StillSource> StillSource for PosterPictures<S> {
    fn still(&self, source: &Input) -> Option<Rgba8> {
        self.cached.still(source)
    }

    fn poster(&self, source: &Input, sniffed: &Sniffed) -> Option<Rgba8> {
        self.cached
            .poster(source, sniffed)
            .or_else(|| self.plugins.poster_pixels(source, sniffed))
    }
}
