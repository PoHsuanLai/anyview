//! How many frames an animated file holds, counted from its container without decoding a pixel:
//! decoding every frame to count them is how a few kilobytes of GIF cost minutes.
//!
//! GIF is walked block by block, which is linear in the file. APNG states its count in `acTL` and
//! animated WebP has one `ANMF` chunk per frame.

use image::ImageFormat;

/// The number of frames in `bytes`, a file of `format`, or `None` when its container does not say.
pub(crate) fn of(bytes: &[u8], format: ImageFormat) -> Option<u32> {
    if format == ImageFormat::Gif {
        gif(bytes)
    } else if format == ImageFormat::Png {
        png(bytes)
    } else if format == ImageFormat::WebP {
        webp(bytes)
    } else {
        None
    }
}

/// Skips the data sub-blocks that start at `at`; the position after the zero-length block.
fn sub_blocks(bytes: &[u8], mut at: usize) -> Option<usize> {
    loop {
        let length = usize::from(*bytes.get(at)?);
        at += 1 + length;
        if length == 0 {
            return Some(at);
        }
    }
}

/// The size in bytes of a colour table that the packed flags `flags` announce, or none.
fn table_size(flags: u8, present: u8) -> usize {
    if flags & present == 0 {
        0
    } else {
        3 * (2_usize << (flags & 7))
    }
}

fn gif(bytes: &[u8]) -> Option<u32> {
    let screen_flags = *bytes.get(10)?;
    let mut at = 13 + table_size(screen_flags, 0x80);
    let mut count: u32 = 0;
    loop {
        match *bytes.get(at)? {
            0x3B => return Some(count),
            0x21 => at = sub_blocks(bytes, at + 2)?,
            0x2C => {
                let flags = *bytes.get(at + 9)?;
                // The image descriptor, its local colour table, the code size, then the data.
                at = sub_blocks(bytes, at + 10 + table_size(flags, 0x80) + 1)?;
                count = count.saturating_add(1);
            }
            _ => return Some(count),
        }
    }
}

fn word(bytes: &[u8], at: usize) -> Option<u32> {
    let raw = bytes.get(at..at.checked_add(4)?)?;
    Some(u32::from_be_bytes(<[u8; 4]>::try_from(raw).ok()?))
}

/// The chunks come before the first `IDAT`, and `acTL` is among them.
fn png(bytes: &[u8]) -> Option<u32> {
    let mut at = 8;
    loop {
        let length = usize::try_from(word(bytes, at)?).ok()?;
        match bytes.get(at + 4..at + 8)? {
            b"acTL" => return word(bytes, at + 8),
            b"IDAT" => return None,
            _ => at = at.checked_add(length)?.checked_add(12)?,
        }
    }
}

/// The chunks follow the twelve bytes of the RIFF header, each padded to an even length.
fn webp(bytes: &[u8]) -> Option<u32> {
    let mut at = 12;
    let mut count: u32 = 0;
    while let (Some(name), Some(raw)) = (bytes.get(at..at + 4), bytes.get(at + 4..at + 8)) {
        let length = usize::try_from(u32::from_le_bytes(<[u8; 4]>::try_from(raw).ok()?)).ok()?;
        if name == b"ANMF" {
            count = count.saturating_add(1);
        }
        at = at
            .checked_add(length)?
            .checked_add(length & 1)?
            .checked_add(8)?;
    }
    (count > 0).then_some(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Frame, RgbaImage, codecs::gif::GifEncoder};

    fn gif_of(frames: usize) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut encoder = GifEncoder::new(&mut out);
            for grey in 0..frames {
                let picture = RgbaImage::from_pixel(8, 6, image::Rgba([grey as u8; 4]));
                encoder.encode_frame(Frame::new(picture)).unwrap();
            }
        }
        out
    }

    #[test]
    fn a_gif_is_counted_from_its_blocks() {
        for frames in [1, 2, 3, 40] {
            assert_eq!(of(&gif_of(frames), ImageFormat::Gif), Some(frames as u32));
        }
    }

    #[test]
    fn a_damaged_container_counts_what_it_has_or_nothing() {
        let gif = gif_of(3);
        // name, bytes, format, count
        let cases: Vec<(&str, Vec<u8>, ImageFormat, Option<u32>)> = vec![
            (
                "a gif cut in its first frame",
                gif[..40].to_vec(),
                ImageFormat::Gif,
                None,
            ),
            (
                "a gif that ends in junk",
                [&gif[..gif.len() - 1], &[7][..]].concat(),
                ImageFormat::Gif,
                Some(3),
            ),
            ("a header only", b"GIF89a".to_vec(), ImageFormat::Gif, None),
            ("empty", Vec::new(), ImageFormat::Gif, None),
            (
                "a png with no acTL",
                b"\x89PNG\r\n\x1a\n".to_vec(),
                ImageFormat::Png,
                None,
            ),
            (
                "a jpeg is not animated",
                vec![0xFF, 0xD8],
                ImageFormat::Jpeg,
                None,
            ),
        ];
        for (name, bytes, format, want) in cases {
            assert_eq!(of(&bytes, format), want, "{name}");
        }
    }

    #[test]
    fn a_png_states_its_frames_and_a_webp_has_a_chunk_each() {
        let mut apng = b"\x89PNG\r\n\x1a\n".to_vec();
        apng.extend([0, 0, 0, 13]);
        apng.extend(b"IHDR");
        apng.extend([0; 13 + 4]);
        apng.extend([0, 0, 0, 8]);
        apng.extend(b"acTL");
        apng.extend(7u32.to_be_bytes());
        apng.extend(0u32.to_be_bytes());
        assert_eq!(of(&apng, ImageFormat::Png), Some(7));

        let mut webp = b"RIFF\0\0\0\0WEBP".to_vec();
        for _ in 0..4 {
            webp.extend(b"ANMF");
            webp.extend(3u32.to_le_bytes());
            webp.extend([0; 4]);
        }
        assert_eq!(of(&webp, ImageFormat::WebP), Some(4));
    }
}
