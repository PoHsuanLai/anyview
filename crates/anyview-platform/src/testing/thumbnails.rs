use super::locked;
use crate::error::PlatformError;
use crate::thumbnail::{ThumbPixels, ThumbSize, ThumbnailCache};
use anyview_core::{FilePath, FileStamp};
use std::sync::{Arc, Mutex};

type Key = (FilePath, FileStamp, ThumbSize);

/// A [`ThumbnailCache`] in memory: a stored thumbnail is found again only for the same file,
/// version and size, as on disk.
#[derive(Debug, Clone, Default)]
pub struct FakeThumbnails {
    stored: Arc<Mutex<Vec<(Key, ThumbPixels)>>>,
}

impl FakeThumbnails {
    /// How many thumbnails are held.
    pub fn len(&self) -> usize {
        locked(&self.stored).len()
    }

    /// Whether none is held.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl ThumbnailCache for FakeThumbnails {
    fn lookup(
        &self,
        file: &FilePath,
        stamp: &FileStamp,
        size: ThumbSize,
    ) -> Result<Option<ThumbPixels>, PlatformError> {
        let key = (file.clone(), *stamp, size);
        Ok(locked(&self.stored)
            .iter()
            .find(|(held, _)| *held == key)
            .map(|(_, pixels)| pixels.clone()))
    }

    fn store(
        &self,
        file: &FilePath,
        stamp: &FileStamp,
        size: ThumbSize,
        pixels: &ThumbPixels,
    ) -> Result<(), PlatformError> {
        let key = (file.clone(), *stamp, size);
        let mut stored = locked(&self.stored);
        stored.retain(|(held, _)| held.0 != key.0 || held.2 != key.2);
        stored.push((key, pixels.clone()));
        Ok(())
    }
}
