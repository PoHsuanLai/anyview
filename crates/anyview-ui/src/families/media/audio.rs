//! What an audio file with no picture shows: a card of one accent colour, chosen from what the
//! file says of itself so the same album always has the same colour, and its title large.

use ds::prelude::Accent;
use ds_core::word::Word;

/// The accent an album is shown in, from its title, artist and album tags (and the file's name
/// when it has none). A fixed hash, so the colour is the same on every run.
pub(super) fn accent_for(parts: &[&str]) -> Accent {
    let mut hash: u32 = 0x811c_9dc5;
    for part in parts {
        for byte in part.bytes().chain(std::iter::once(0)) {
            hash ^= u32::from(byte);
            hash = hash.wrapping_mul(0x0100_0193);
        }
    }
    let all = Accent::ALL;
    all[hash as usize % all.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_album_always_has_the_same_colour_and_others_differ() {
        let one = accent_for(&["Blue Train", "John Coltrane", "Blue Train"]);
        assert_eq!(
            one,
            accent_for(&["Blue Train", "John Coltrane", "Blue Train"])
        );
        let others: Vec<Accent> = ["a", "b", "c", "d", "e", "f", "g", "h"]
            .iter()
            .map(|name| accent_for(&[name]))
            .collect();
        let mut distinct = others.clone();
        distinct.dedup();
        assert!(distinct.len() > 1, "eight titles all hashed to one accent");
    }
}
