//! The density a picture says it should be shown at, in dots an inch, from whichever header
//! carries it: EXIF, a PNG's `pHYs` chunk or a JPEG's JFIF block.

use crate::exif::Ratio;
use anyview_core::{FormatDetail, RasterFormat, Sniffed};

/// What a density is counted per.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Unit {
    /// Dots an inch.
    Inch,
    /// Dots a centimetre.
    Centimetre,
}

/// Dots an inch across and down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Resolution {
    /// Dots an inch along the width.
    pub horizontal: u32,
    /// Dots an inch along the height.
    pub vertical: u32,
}

impl Resolution {
    /// A density of `horizontal` and `vertical` dots per `unit`; `None` when either is zero.
    pub(crate) fn per_unit(horizontal: Ratio, vertical: Ratio, unit: Unit) -> Option<Self> {
        let dpi = |ratio: Ratio| {
            let (numerator, denominator) = match unit {
                Unit::Inch => (
                    u64::from(ratio.numerator) * 100,
                    u64::from(ratio.denominator) * 100,
                ),
                Unit::Centimetre => (
                    u64::from(ratio.numerator) * 254,
                    u64::from(ratio.denominator) * 100,
                ),
            };
            let dots = u32::try_from((numerator + denominator / 2) / denominator).ok()?;
            (dots > 0).then_some(dots)
        };
        Some(Resolution {
            horizontal: dpi(horizontal)?,
            vertical: dpi(vertical)?,
        })
    }

    /// A density of `horizontal` and `vertical` dots a metre; `None` when either rounds to zero
    /// dots an inch.
    fn per_metre(horizontal: u32, vertical: u32) -> Option<Self> {
        let dpi = |metre: u32| {
            let dots = u32::try_from((u64::from(metre) * 254 + 5_000) / 10_000).ok()?;
            (dots > 0).then_some(dots)
        };
        Some(Resolution {
            horizontal: dpi(horizontal)?,
            vertical: dpi(vertical)?,
        })
    }

    /// `300 dpi`, or `300 × 72 dpi` when the two directions differ.
    pub fn text(self) -> String {
        if self.horizontal == self.vertical {
            format!("{} dpi", self.horizontal)
        } else {
            format!("{} × {} dpi", self.horizontal, self.vertical)
        }
    }
}

/// The density a PNG's `pHYs` chunk or a JPEG's JFIF block states; `None` for other formats, for
/// a file without one, and for a density that only states an aspect ratio.
pub(crate) fn of_header(bytes: &[u8], sniffed: &Sniffed) -> Option<Resolution> {
    let FormatDetail::Raster(format) = sniffed.detail() else {
        return None;
    };
    if *format == RasterFormat::Png {
        png(bytes)
    } else if *format == RasterFormat::Jpeg {
        jfif(bytes)
    } else {
        None
    }
}

/// The `pHYs` chunk: pixels a metre across and down, then the unit byte (1 is the metre).
fn png(bytes: &[u8]) -> Option<Resolution> {
    let mut rest = bytes.strip_prefix(b"\x89PNG\r\n\x1a\n")?;
    while rest.len() >= 12 {
        let length = usize::try_from(u32::from_be_bytes(rest[..4].try_into().ok()?)).ok()?;
        let kind = &rest[4..8];
        let data = rest.get(8..8usize.checked_add(length)?)?;
        match kind {
            b"pHYs" if data.len() == 9 && data[8] == 1 => {
                let metre = |at: usize| {
                    u32::from_be_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]])
                };
                return Resolution::per_metre(metre(0), metre(4));
            }
            b"IDAT" | b"IEND" => return None,
            _ => {}
        }
        rest = rest.get(12usize.checked_add(length)?..)?;
    }
    None
}

/// The JFIF block among a JPEG's first segments: units (1 dots an inch, 2 a centimetre), then the
/// densities.
fn jfif(bytes: &[u8]) -> Option<Resolution> {
    let mut rest = bytes.strip_prefix(b"\xFF\xD8")?;
    // Segments are `FF marker, length (including itself), payload`; the picture starts at SOS.
    while let [0xFF, marker, high, low, ..] = *rest {
        let length = usize::from(u16::from_be_bytes([high, low]));
        let payload = rest.get(4..2 + length)?;
        if marker == 0xE0
            && let Some(block) = payload.strip_prefix(b"JFIF\0")
        {
            return density(block);
        }
        if marker == 0xDA {
            return None;
        }
        rest = rest.get(2 + length..)?;
    }
    None
}

/// The density in a JFIF block after its signature: version (2 bytes), units, then two densities.
fn density(block: &[u8]) -> Option<Resolution> {
    let [_, _, units, x_high, x_low, y_high, y_low, ..] = *block else {
        return None;
    };
    let whole = |high: u8, low: u8| Ratio {
        numerator: u32::from(u16::from_be_bytes([high, low])),
        denominator: 1,
    };
    let unit = match units {
        1 => Unit::Inch,
        2 => Unit::Centimetre,
        _ => return None,
    };
    Resolution::per_unit(whole(x_high, x_low), whole(y_high, y_low), unit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn densities_read_as_dots_an_inch() {
        let whole = |n: u32| Ratio {
            numerator: n,
            denominator: 1,
        };
        let got = Resolution::per_unit(whole(300), whole(300), Unit::Inch).unwrap();
        assert_eq!(got.text(), "300 dpi");
        let got = Resolution::per_unit(whole(118), whole(118), Unit::Centimetre).unwrap();
        assert_eq!(got.text(), "300 dpi");
        let got = Resolution::per_unit(whole(300), whole(72), Unit::Inch).unwrap();
        assert_eq!(got.text(), "300 × 72 dpi");
        assert_eq!(Resolution::per_unit(whole(0), whole(72), Unit::Inch), None);
    }

    #[test]
    fn a_png_phys_chunk_gives_its_density() {
        let chunk = |kind: &[u8; 4], data: &[u8]| {
            let mut bytes = u32::try_from(data.len()).unwrap().to_be_bytes().to_vec();
            bytes.extend_from_slice(kind);
            bytes.extend_from_slice(data);
            bytes.extend_from_slice(&[0; 4]);
            bytes
        };
        let phys = |per_metre: u32, unit: u8| {
            let mut data = per_metre.to_be_bytes().to_vec();
            data.extend_from_slice(&per_metre.to_be_bytes());
            data.push(unit);
            let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
            png.extend(chunk(b"IHDR", &[0; 13]));
            png.extend(chunk(b"pHYs", &data));
            png.extend(chunk(b"IDAT", &[]));
            png
        };
        assert_eq!(
            png(&phys(11_811, 1)).map(Resolution::text).as_deref(),
            Some("300 dpi")
        );
        assert_eq!(
            png(&phys(2_835, 1)).map(Resolution::text).as_deref(),
            Some("72 dpi")
        );
        assert_eq!(png(&phys(2_835, 0)), None);
        assert_eq!(png(b"\x89PNG\r\n\x1a\n"), None);
    }

    #[test]
    fn a_jfif_block_gives_its_density_only_when_it_has_units() {
        let block = |units: u8, density: u16| {
            let mut bytes = b"\xFF\xD8\xFF\xE0\0\x0EJFIF\0\x01\x01".to_vec();
            bytes.push(units);
            bytes.extend(density.to_be_bytes());
            bytes.extend(density.to_be_bytes());
            bytes
        };
        assert_eq!(
            jfif(&block(1, 96)).map(Resolution::text).as_deref(),
            Some("96 dpi")
        );
        assert_eq!(
            jfif(&block(2, 38)).map(Resolution::text).as_deref(),
            Some("97 dpi")
        );
        assert_eq!(jfif(&block(0, 1)), None);
        assert_eq!(jfif(b"\xFF\xD8\xFF\xE1\0\x02"), None);
        // Another segment may come first.
        let mut after_exif = b"\xFF\xD8\xFF\xE1\0\x04ab".to_vec();
        after_exif.extend_from_slice(&block(1, 300)[2..]);
        assert_eq!(
            jfif(&after_exif).map(Resolution::text).as_deref(),
            Some("300 dpi")
        );
    }
}
