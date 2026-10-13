//! The merged picture of a Photoshop document, read from the file itself.
//!
//! A PSD is a 26-byte header, then three length-prefixed sections that are skipped (the colour
//! mode data, the image resources, the layer and mask information), then the composite: a
//! compression word and the channels as planes, raw or PackBits. A PSB (version 2) is the same
//! with an eight-byte length for the layer section and four-byte row lengths in the RLE table.
//!
//! The `psd` crate indexes the layer section before it reads the composite and panics on files
//! other writers make, so the composite is read here first; the crate is only asked about what
//! this reader does not know (ZIP compression, 1 and 32 bits, Lab, multichannel).

/// Why the composite could not be read.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Gap {
    /// A valid file of a kind this reader does not handle.
    Unsupported,
    /// The bytes end early or contradict the header.
    Damaged(&'static str),
}

/// The big-endian number of `len` bytes (at most 8) at `at`.
fn be(bytes: &[u8], at: usize, len: usize) -> Option<u64> {
    let end = at.checked_add(len)?;
    bytes.get(at..end)?.iter().try_fold(0_u64, |number, byte| {
        Some(number.checked_shl(8)? | u64::from(*byte))
    })
}

/// A number at `at` as a `usize`, or damage.
fn size_at(bytes: &[u8], at: usize, len: usize) -> Result<usize, Gap> {
    be(bytes, at, len)
        .and_then(|number| usize::try_from(number).ok())
        .ok_or(Gap::Damaged("a Photoshop header is cut short"))
}

/// The colour channels of a mode, or `Unsupported`.
fn colour_channels(mode: u64) -> Result<usize, Gap> {
    match mode {
        1 | 2 => Ok(1),
        3 => Ok(3),
        4 => Ok(4),
        _ => Err(Gap::Unsupported),
    }
}

/// Unpacks one PackBits row into `out`, which it must fill exactly.
fn unpack_bits(source: &[u8], out: &mut [u8]) -> bool {
    let (mut from, mut to) = (0_usize, 0_usize);
    while to < out.len() {
        let Some(&code) = source.get(from) else {
            return false;
        };
        from += 1;
        let code = i8::from_be_bytes([code]);
        if code == i8::MIN {
            continue;
        }
        if code >= 0 {
            let run = usize::from(code.unsigned_abs()) + 1;
            let (Some(literal), Some(slot)) =
                (source.get(from..from + run), out.get_mut(to..to + run))
            else {
                return false;
            };
            slot.copy_from_slice(literal);
            from += run;
            to += run;
        } else {
            let run = usize::from(code.unsigned_abs()) + 1;
            let (Some(&value), Some(slot)) = (source.get(from), out.get_mut(to..to + run)) else {
                return false;
            };
            slot.fill(value);
            from += 1;
            to += run;
        }
    }
    true
}

/// The straight RGBA8 pixels of the composite of the Photoshop file `bytes`, and its size.
///
/// 8 and 16 bits (the high byte is kept), Grayscale, Indexed, RGB and CMYK, raw or RLE. The extra
/// channel after the colour is the transparency when the layer count is negative, which is how
/// Photoshop says the merged picture has transparency; otherwise extra channels are saved
/// selections and are ignored.
pub(super) fn composite(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), Gap> {
    let damaged = Gap::Damaged;
    if bytes.get(..4) != Some(b"8BPS") {
        return Err(damaged("not a Photoshop file"));
    }
    let version = size_at(bytes, 4, 2)?;
    let word = match version {
        1 => 4,
        2 => 8,
        _ => return Err(Gap::Unsupported),
    };
    let channels = size_at(bytes, 12, 2)?;
    let height = size_at(bytes, 14, 4)?;
    let width = size_at(bytes, 18, 4)?;
    let depth = size_at(bytes, 22, 2)?;
    let mode = be(bytes, 24, 2).ok_or(damaged("a Photoshop header is cut short"))?;
    let colour = colour_channels(mode)?;
    let sample = match depth {
        8 => 1,
        16 => 2,
        _ => return Err(Gap::Unsupported),
    };
    if channels < colour || width == 0 || height == 0 {
        return Err(damaged("the Photoshop header is not a picture"));
    }

    // The colour mode data (a palette for Indexed), the resources and the layer section.
    let mut at = 26;
    let palette_len = size_at(bytes, at, 4)?;
    at += 4;
    let palette = bytes.get(at..at.saturating_add(palette_len));
    at = at.saturating_add(palette_len);
    let resources = size_at(bytes, at, 4)?;
    at = at.saturating_add(4).saturating_add(resources);
    let layers = size_at(bytes, at, word)?;
    at += word;
    // The layer information starts with its own length, then the layer count.
    let transparent = layers > word + 2
        && size_at(bytes, at, word)? >= 2
        && be(bytes, at + word, 2).is_some_and(|count| count & 0x8000 != 0);
    at = at.saturating_add(layers);

    let used = colour + usize::from(transparent && channels > colour);
    let row = width
        .checked_mul(sample)
        .ok_or(damaged("the picture is too wide"))?;
    let compression = be(bytes, at, 2).ok_or(damaged("the picture data is missing"))?;
    at += 2;
    let mut planes = vec![vec![0_u8; 0]; used];
    match compression {
        0 => {
            let plane = row
                .checked_mul(height)
                .ok_or(damaged("the picture is too tall"))?;
            for (index, slot) in planes.iter_mut().enumerate() {
                let start = at.saturating_add(index.saturating_mul(plane));
                let data = bytes
                    .get(start..start.saturating_add(plane))
                    .ok_or(damaged("the picture data is cut short"))?;
                *slot = data.to_vec();
            }
        }
        1 => {
            let entry = if version == 2 { 4 } else { 2 };
            let rows = channels
                .checked_mul(height)
                .ok_or(damaged("the picture is too tall"))?;
            let mut data = at.saturating_add(rows.saturating_mul(entry));
            for index in 0..rows {
                let length = size_at(bytes, at + index * entry, entry)?;
                let packed = bytes
                    .get(data..data.saturating_add(length))
                    .ok_or(damaged("the picture data is cut short"))?;
                data += length;
                let (channel, line) = (index / height, index % height);
                if let Some(plane) = planes.get_mut(channel) {
                    if plane.is_empty() {
                        plane.resize(row * height, 0);
                    }
                    let out = plane
                        .get_mut(line * row..(line + 1) * row)
                        .ok_or(damaged("the picture is too tall"))?;
                    if !unpack_bits(packed, out) {
                        return Err(damaged("a row of the picture is damaged"));
                    }
                }
            }
        }
        _ => return Err(Gap::Unsupported),
    }

    let palette = match (mode, palette) {
        (2, Some(table)) if table.len() == 768 && sample == 1 => Some(table),
        (2, _) => return Err(Gap::Unsupported),
        _ => None,
    };
    let pixels = width
        .checked_mul(height)
        .ok_or(damaged("the picture is too large"))?;
    let mut out = Vec::with_capacity(pixels.saturating_mul(4));
    let value = |plane: usize, pixel: usize| -> u8 {
        planes
            .get(plane)
            .and_then(|samples| samples.get(pixel * sample))
            .copied()
            .unwrap_or(0)
    };
    for pixel in 0..pixels {
        let rgb = match (mode, palette) {
            (1, _) => [value(0, pixel); 3],
            (2, Some(table)) => {
                let index = usize::from(value(0, pixel));
                let entry = |channel: usize| table.get(channel * 256 + index).copied().unwrap_or(0);
                [entry(0), entry(1), entry(2)]
            }
            (4, _) => {
                // Photoshop stores CMYK inverted: 255 is no ink.
                let key = u32::from(value(3, pixel));
                let mix = |ink: u8| ((u32::from(ink) * key + 127) / 255) as u8; // at most 255
                [
                    mix(value(0, pixel)),
                    mix(value(1, pixel)),
                    mix(value(2, pixel)),
                ]
            }
            _ => [value(0, pixel), value(1, pixel), value(2, pixel)],
        };
        let alpha = if used > colour {
            value(colour, pixel)
        } else {
            255
        };
        out.extend_from_slice(&[rgb[0], rgb[1], rgb[2], alpha]);
    }
    let to = |number: usize| u32::try_from(number).map_err(|_| damaged("the picture is too large"));
    Ok((to(width)?, to(height)?, out))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One PSD or PSB, built from its parts.
    struct Doc {
        version: u16,
        channels: u16,
        width: u32,
        height: u32,
        depth: u16,
        mode: u16,
        palette: Vec<u8>,
        /// The layer information: its bytes after the section length, which is written for it.
        layers: Vec<u8>,
        compression: u16,
        data: Vec<u8>,
    }

    impl Doc {
        fn rgb(width: u32, height: u32, channels: u16, depth: u16) -> Doc {
            Doc {
                version: 1,
                channels,
                width,
                height,
                depth,
                mode: 3,
                palette: Vec::new(),
                layers: Vec::new(),
                compression: 0,
                data: Vec::new(),
            }
        }

        fn bytes(&self) -> Vec<u8> {
            let mut out = b"8BPS".to_vec();
            out.extend(self.version.to_be_bytes());
            out.extend([0; 6]);
            out.extend(self.channels.to_be_bytes());
            out.extend(self.height.to_be_bytes());
            out.extend(self.width.to_be_bytes());
            out.extend(self.depth.to_be_bytes());
            out.extend(self.mode.to_be_bytes());
            out.extend(u32::try_from(self.palette.len()).unwrap().to_be_bytes());
            out.extend(&self.palette);
            out.extend(0_u32.to_be_bytes()); // no image resources
            let length = self.layers.len() as u64;
            if self.version == 2 {
                out.extend(length.to_be_bytes());
            } else {
                out.extend(u32::try_from(length).unwrap().to_be_bytes());
            }
            out.extend(&self.layers);
            out.extend(self.compression.to_be_bytes());
            out.extend(&self.data);
            out
        }
    }

    /// Layer information of `count` layers (negative: the merged picture has transparency) and
    /// some bytes that stand for the layers themselves.
    fn layer_info(count: i16, version: u16) -> Vec<u8> {
        let mut info = count.to_be_bytes().to_vec();
        info.extend([0xAB; 9]);
        let mut out = Vec::new();
        if version == 2 {
            out.extend((info.len() as u64).to_be_bytes());
        } else {
            out.extend((info.len() as u32).to_be_bytes());
        }
        out.extend(info);
        out
    }

    /// RLE data for `rows` (channel after channel, row after row): the length table, then each row
    /// as one repeat run when its bytes are all alike and as literal runs otherwise.
    fn rle(rows: &[&[u8]], version: u16) -> Vec<u8> {
        let packed: Vec<Vec<u8>> = rows
            .iter()
            .map(|row| {
                if row.len() > 1 && row.iter().all(|byte| *byte == row[0]) {
                    vec![(1 - row.len() as i16) as u8, row[0]]
                } else {
                    let mut line = vec![(row.len() - 1) as u8];
                    line.extend(*row);
                    line
                }
            })
            .collect();
        let mut out = Vec::new();
        for line in &packed {
            if version == 2 {
                out.extend((line.len() as u32).to_be_bytes());
            } else {
                out.extend((line.len() as u16).to_be_bytes());
            }
        }
        for line in packed {
            out.extend(line);
        }
        out
    }

    #[test]
    fn a_composite_reads_whatever_the_layer_section_holds() {
        // name, doc, rgba of its two pixels
        let cases: Vec<(&str, Doc, [u8; 8])> = vec![
            (
                "rgb, raw",
                Doc {
                    data: vec![10, 20, 30, 40, 50, 60],
                    ..Doc::rgb(2, 1, 3, 8)
                },
                [10, 30, 50, 255, 20, 40, 60, 255],
            ),
            (
                "rgb, layers skipped",
                Doc {
                    layers: layer_info(2, 1),
                    data: vec![10, 20, 30, 40, 50, 60],
                    ..Doc::rgb(2, 1, 3, 8)
                },
                [10, 30, 50, 255, 20, 40, 60, 255],
            ),
            (
                "rgb, a saved selection is not transparency",
                Doc {
                    layers: layer_info(1, 1),
                    data: vec![10, 20, 30, 40, 50, 60, 7, 7],
                    ..Doc::rgb(2, 1, 4, 8)
                },
                [10, 30, 50, 255, 20, 40, 60, 255],
            ),
            (
                "rgba, a negative layer count says the fourth channel is transparency",
                Doc {
                    layers: layer_info(-1, 1),
                    data: vec![10, 20, 30, 40, 50, 60, 70, 80],
                    ..Doc::rgb(2, 1, 4, 8)
                },
                [10, 30, 50, 70, 20, 40, 60, 80],
            ),
            (
                "rgba, 16 bits keep the high byte",
                Doc {
                    layers: layer_info(-1, 1),
                    data: vec![
                        0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x11, 0x22, 0x33, 0x44,
                        0x55, 0x66, 0x77, 0x88,
                    ],
                    ..Doc::rgb(2, 1, 4, 16)
                },
                [0x12, 0x9A, 0x11, 0x55, 0x56, 0xDE, 0x33, 0x77],
            ),
            (
                "rgb, rle with a repeat and a literal run",
                Doc {
                    compression: 1,
                    data: rle(&[&[9, 9], &[1, 2], &[3, 3]], 1),
                    ..Doc::rgb(2, 1, 3, 8)
                },
                [9, 1, 3, 255, 9, 2, 3, 255],
            ),
            (
                "psb, rle with four-byte row lengths and an eight-byte layer section",
                Doc {
                    version: 2,
                    layers: layer_info(-1, 2),
                    compression: 1,
                    data: rle(&[&[9, 9], &[1, 2], &[3, 3], &[5, 6]], 2),
                    ..Doc::rgb(2, 1, 4, 8)
                },
                [9, 1, 3, 5, 9, 2, 3, 6],
            ),
            (
                "grayscale",
                Doc {
                    mode: 1,
                    data: vec![40, 200],
                    ..Doc::rgb(2, 1, 1, 8)
                },
                [40, 40, 40, 255, 200, 200, 200, 255],
            ),
            (
                "cmyk is stored inverted",
                Doc {
                    mode: 4,
                    // pixel 0: no ink at all; pixel 1: full cyan, no other ink
                    data: vec![255, 0, 255, 255, 255, 255, 255, 255],
                    ..Doc::rgb(2, 1, 4, 8)
                },
                [255, 255, 255, 255, 0, 255, 255, 255],
            ),
            (
                "indexed",
                Doc {
                    mode: 2,
                    palette: (0..768)
                        .map(|n| u8::try_from((n / 256 * 10 + n % 256) % 256).unwrap())
                        .collect(),
                    data: vec![1, 2],
                    ..Doc::rgb(2, 1, 1, 8)
                },
                [1, 11, 21, 255, 2, 12, 22, 255],
            ),
        ];
        for (name, doc, want) in cases {
            let (width, height, pixels) = composite(&doc.bytes()).unwrap_or_else(|gap| {
                panic!("{name}: {gap:?}");
            });
            assert_eq!((width, height), (2, 1), "{name}: size");
            assert_eq!(pixels, want, "{name}");
        }
    }

    #[test]
    fn what_cannot_be_read_is_said_to_be_unsupported_or_damaged() {
        let raw = Doc {
            data: vec![0; 6],
            ..Doc::rgb(2, 1, 3, 8)
        };
        let mut cut = raw.bytes();
        cut.truncate(cut.len() - 1);
        let mut bad_row = Doc {
            compression: 1,
            data: rle(&[&[9, 9], &[1, 2], &[3, 3]], 1),
            ..Doc::rgb(2, 1, 3, 8)
        }
        .bytes();
        let last = bad_row.len() - 1;
        bad_row[last - 1] = 0x7F; // a literal run of 128 bytes in a row of two
        // name, bytes, gap
        let cases: Vec<(&str, Vec<u8>, Gap)> = vec![
            (
                "zip",
                Doc {
                    compression: 2,
                    ..Doc::rgb(2, 1, 3, 8)
                }
                .bytes(),
                Gap::Unsupported,
            ),
            (
                "32 bits",
                Doc {
                    data: vec![0; 24],
                    ..Doc::rgb(2, 1, 3, 32)
                }
                .bytes(),
                Gap::Unsupported,
            ),
            (
                "lab",
                Doc {
                    mode: 9,
                    ..Doc::rgb(2, 1, 3, 8)
                }
                .bytes(),
                Gap::Unsupported,
            ),
            (
                "pixels cut short",
                cut,
                Gap::Damaged("the picture data is cut short"),
            ),
            (
                "a run past the row",
                bad_row,
                Gap::Damaged("a row of the picture is damaged"),
            ),
            (
                "not a picture",
                Doc::rgb(0, 1, 3, 8).bytes(),
                Gap::Damaged("the Photoshop header is not a picture"),
            ),
        ];
        for (name, bytes, gap) in cases {
            assert_eq!(composite(&bytes).unwrap_err(), gap, "{name}");
        }
    }
}
