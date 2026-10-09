//! The first frame of a picture from the thumbnail cache the desktop shares: the small picture
//! some program made of the file earlier, shown while the file decodes.

use anyview_core::Input;
use anyview_image::Rgba8;
use anyview_peek::StillSource;
use anyview_platform::{ThumbSize, ThumbnailCache};

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
