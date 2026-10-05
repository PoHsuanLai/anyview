//! How many times an animated file asks to be played, read from its container: the `image`
//! crate's frame iterators do not report it.
//!
//! GIF stores it in the NETSCAPE application extension as the repeats after the first play (zero
//! meaning forever), and a GIF without the extension plays once, as browsers show it. APNG stores
//! the plays in its `acTL` chunk and animated WebP in its `ANIM` chunk, zero meaning forever in
//! both; a file missing the chunk is read as forever.

use image::ImageFormat;
use std::num::NonZeroU32;

/// How many times an animation runs through its frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Plays {
    /// Until the person stops it.
    Forever,
    /// This many runs, then it holds on the last frame.
    Times(NonZeroU32),
}

impl Plays {
    fn counted(count: u32) -> Plays {
        NonZeroU32::new(count).map_or(Plays::Forever, Plays::Times)
    }
}

/// How often the animation in `bytes`, a file of `format`, asks to play.
pub(crate) fn plays_of(bytes: &[u8], format: ImageFormat) -> Plays {
    if format == ImageFormat::Gif {
        gif(bytes)
    } else if format == ImageFormat::Png {
        png(bytes).unwrap_or(Plays::Forever)
    } else if format == ImageFormat::WebP {
        webp(bytes).unwrap_or(Plays::Forever)
    } else {
        Plays::Forever
    }
}

/// The NETSCAPE extension sits before the first frame, after the screen descriptor and a global
/// colour table of at most 768 bytes, so only the head of the file is searched.
fn gif(bytes: &[u8]) -> Plays {
    const NAME: &[u8] = b"NETSCAPE2.0";
    let head = &bytes[..bytes.len().min(1024)];
    let Some(at) = head.windows(NAME.len()).position(|window| window == NAME) else {
        return Plays::Times(NonZeroU32::MIN);
    };
    match bytes.get(at + NAME.len()..at + NAME.len() + 4) {
        Some(&[3, 1, low, high]) => {
            let repeats = u32::from(u16::from_le_bytes([low, high]));
            if repeats == 0 {
                Plays::Forever
            } else {
                Plays::counted(repeats.saturating_add(1))
            }
        }
        _ => Plays::Times(NonZeroU32::MIN),
    }
}

fn word(bytes: &[u8], at: usize) -> Option<u32> {
    let raw = bytes.get(at..at.checked_add(4)?)?;
    Some(u32::from_be_bytes(<[u8; 4]>::try_from(raw).ok()?))
}

/// The chunks come before the first `IDAT`, and `acTL` is among them.
fn png(bytes: &[u8]) -> Option<Plays> {
    let mut at = 8;
    loop {
        let length = usize::try_from(word(bytes, at)?).ok()?;
        match bytes.get(at + 4..at + 8)? {
            b"acTL" => return Some(Plays::counted(word(bytes, at + 12)?)),
            b"IDAT" => return None,
            _ => at = at.checked_add(length)?.checked_add(12)?,
        }
    }
}

/// The chunks follow the twelve bytes of the RIFF header, each padded to an even length.
fn webp(bytes: &[u8]) -> Option<Plays> {
    let mut at = 12;
    loop {
        let name = bytes.get(at..at + 4)?;
        let raw = bytes.get(at + 4..at + 8)?;
        let length = usize::try_from(u32::from_le_bytes(<[u8; 4]>::try_from(raw).ok()?)).ok()?;
        if name == b"ANIM" {
            let loops = bytes.get(at + 12..at + 14)?;
            return Some(Plays::counted(u32::from(u16::from_le_bytes([
                loops[0], loops[1],
            ]))));
        }
        at = at
            .checked_add(length)?
            .checked_add(length & 1)?
            .checked_add(8)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn times(count: u32) -> Plays {
        match NonZeroU32::new(count) {
            Some(count) => Plays::Times(count),
            None => panic!("a count of runs is at least one"),
        }
    }

    fn gif_with(extension: &[u8]) -> Vec<u8> {
        [b"GIF89a".as_slice(), &[0; 7], extension, &[0x3b]].concat()
    }

    fn netscape(low: u8, high: u8) -> Vec<u8> {
        [
            &[0x21, 0xff, 0x0b][..],
            b"NETSCAPE2.0",
            &[3, 1, low, high, 0],
        ]
        .concat()
    }

    fn apng(plays: u32) -> Vec<u8> {
        let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
        out.extend([0, 0, 0, 13]);
        out.extend(b"IHDR");
        out.extend([0; 13 + 4]);
        out.extend([0, 0, 0, 8]);
        out.extend(b"acTL");
        out.extend(3u32.to_be_bytes());
        out.extend(plays.to_be_bytes());
        out.extend([0; 4]);
        out.extend([0, 0, 0, 0]);
        out.extend(b"IDAT");
        out
    }

    fn animated_webp(loops: u16) -> Vec<u8> {
        let mut out = b"RIFF\0\0\0\0WEBP".to_vec();
        out.extend(b"VP8X");
        out.extend(10u32.to_le_bytes());
        out.extend([0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        out.extend(b"ANIM");
        out.extend(6u32.to_le_bytes());
        out.extend([0, 0, 0, 0]);
        out.extend(loops.to_le_bytes());
        out
    }

    #[test]
    fn plays_are_read_from_each_container() {
        // name, format, file, plays
        let cases: Vec<(&str, ImageFormat, Vec<u8>, Plays)> = vec![
            (
                "gif forever",
                ImageFormat::Gif,
                gif_with(&netscape(0, 0)),
                Plays::Forever,
            ),
            (
                "gif two repeats is three runs",
                ImageFormat::Gif,
                gif_with(&netscape(2, 0)),
                times(3),
            ),
            (
                "gif repeats are two bytes",
                ImageFormat::Gif,
                gif_with(&netscape(0, 1)),
                times(257),
            ),
            (
                "gif without the extension plays once",
                ImageFormat::Gif,
                gif_with(&[]),
                times(1),
            ),
            ("apng forever", ImageFormat::Png, apng(0), Plays::Forever),
            ("apng four plays", ImageFormat::Png, apng(4), times(4)),
            (
                "a png with no acTL is forever",
                ImageFormat::Png,
                b"\x89PNG\r\n\x1a\n".to_vec(),
                Plays::Forever,
            ),
            (
                "webp forever",
                ImageFormat::WebP,
                animated_webp(0),
                Plays::Forever,
            ),
            (
                "webp five plays",
                ImageFormat::WebP,
                animated_webp(5),
                times(5),
            ),
            (
                "a truncated webp is forever",
                ImageFormat::WebP,
                b"RIFF".to_vec(),
                Plays::Forever,
            ),
            (
                "a still format is forever",
                ImageFormat::Bmp,
                vec![],
                Plays::Forever,
            ),
        ];
        for (name, format, bytes, want) in cases {
            assert_eq!(plays_of(&bytes, format), want, "{name}");
        }
    }
}
