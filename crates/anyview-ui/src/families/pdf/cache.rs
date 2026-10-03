//! The tiles on the GPU, held under a budget of bytes. A tile is one texture; the cache holds the
//! handles the workers uploaded and decides which to let go when it is over budget: never one the
//! view wants, then the zooms farthest from the one on screen, then the pages farthest from the
//! reader. What it lets go is dropped, and the GPU frees a texture when its last handle goes.

use super::scene::distance;
use anyview_core::{PageIndex, PixelSize};
use anyview_pdf::{TileKey, ZoomBucket};
use ds_blitz::TextureHandle;
use std::collections::HashMap;

/// The most pixel memory the tiles may hold: 192 full tiles.
pub(super) const BUDGET: u64 = 192 * 1024 * 1024;

/// A tile on the GPU.
#[derive(Debug, Clone)]
pub(super) struct Slot {
    pub texture: TextureHandle,
    pub size: PixelSize,
}

impl Slot {
    /// The bytes its texture holds: four to a pixel.
    fn bytes(&self) -> u64 {
        self.size.area().0 * 4
    }
}

/// The tiles held, by key.
#[derive(Debug, Default)]
pub(super) struct TileCache {
    slots: HashMap<TileKey, Slot>,
}

impl TileCache {
    pub(super) fn insert(&mut self, key: TileKey, slot: Slot) {
        self.slots.insert(key, slot);
    }

    pub(super) fn get(&self, key: &TileKey) -> Option<&Slot> {
        self.slots.get(key)
    }

    pub(super) fn contains(&self, key: &TileKey) -> bool {
        self.slots.contains_key(key)
    }

    pub(super) fn keys(&self) -> Vec<TileKey> {
        self.slots.keys().copied().collect()
    }

    /// Let go of what is over `budget`: see [`evictions`].
    pub(super) fn trim(
        &mut self,
        budget: u64,
        current: ZoomBucket,
        reader: PageIndex,
        wanted: &dyn Fn(&TileKey) -> bool,
    ) {
        let held: Vec<(TileKey, u64)> = self
            .slots
            .iter()
            .map(|(key, slot)| (*key, slot.bytes()))
            .collect();
        for key in evictions(&held, budget, current, reader, wanted) {
            self.slots.remove(&key);
        }
    }
}

/// The tiles of `held` (key and bytes) to let go so the rest fit `budget`: none of those `wanted`,
/// the zoom farthest from `current` first, then the page farthest from `reader`. Fewer when the
/// wanted tiles alone are over budget.
pub(super) fn evictions(
    held: &[(TileKey, u64)],
    budget: u64,
    current: ZoomBucket,
    reader: PageIndex,
    wanted: &dyn Fn(&TileKey) -> bool,
) -> Vec<TileKey> {
    let total: u64 = held.iter().map(|(_, bytes)| bytes).sum();
    let mut spare: Vec<&(TileKey, u64)> = held.iter().filter(|(key, _)| !wanted(key)).collect();
    spare.sort_by_key(|(key, _)| {
        (
            std::cmp::Reverse(distance(key.zoom, current)),
            std::cmp::Reverse(key.page.0.abs_diff(reader.0)),
            *key,
        )
    });
    let mut over = total.saturating_sub(budget);
    let mut gone = Vec::new();
    for (key, bytes) in spare {
        if over == 0 {
            break;
        }
        over = over.saturating_sub(*bytes);
        gone.push(*key);
    }
    gone
}
