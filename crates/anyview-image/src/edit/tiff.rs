//! A TIFF turned or mirrored page by page: every page (IFD) is decoded at its own bit depth and
//! sample layout, moved, and written again with its compression, predictor, photometric
//! interpretation, palette and every other tag. The orientation tag is baked into the pixels, so
//! the written pages are upright.
//!
//! A picture the pages of which hold a pointer to another directory (EXIF, GPS, sub-pictures)
//! cannot be rewritten without those pointers going stale, so a one-page file like that is
//! turned through its orientation tag, which changes nothing else; a longer one is not offered.

use super::place::{Grid, swaps_axes};
use super::{Fidelity, Loss, Placing};
use crate::error::ImageError;
use crate::exif::{ExifFacts, with_orientation};
use crate::orientation::ExifOrientation;
use std::io::{Cursor, Seek, Write};
use tiff::decoder::{Decoder, DecodingResult};
use tiff::encoder::colortype::ColorType;
use tiff::encoder::compression::DeflateLevel;
use tiff::encoder::{Compression, TiffEncoder};
use tiff::tags::{ByteOrder, Predictor, SampleFormat, Tag, ValueBuffer};

/// More pages than this is not a document anyone scans.
const MAX_PAGES: usize = 4096;

const ORIENTATION: u16 = 274;
const X_RESOLUTION: u16 = 282;
const Y_RESOLUTION: u16 = 283;
/// The tags the encoder writes itself or that describe where the old strips were: width, length,
/// bits per sample, compression, strip offsets, orientation, samples per pixel, rows per strip,
/// strip byte counts, the predictor, the tile layout and the sample format.
const STRUCTURAL: &[u16] = &[
    256,
    257,
    258,
    259,
    273,
    ORIENTATION,
    277,
    278,
    279,
    317,
    322,
    323,
    324,
    325,
    339,
];
/// Tags that hold the place of another directory, which cannot move with the picture: SubIFDs, the EXIF directory, the GPS directory and the interoperability directory.
const POINTERS: &[u16] = &[330, 34665, 34853, 40965];

/// What a page says about how it is stored.
struct Facts {
    compression: u16,
    predictor: u16,
    photometric: u16,
    depth: u16,
    samples: usize,
    orientation: ExifOrientation,
    points_elsewhere: bool,
    supported: bool,
}

fn tiff_error(error: tiff::TiffError) -> ImageError {
    ImageError::Container {
        reason: error.to_string(),
    }
}

fn facts_of(decoder: &mut Decoder<Cursor<&[u8]>>) -> Result<Facts, ImageError> {
    let unsigned = |decoder: &mut Decoder<Cursor<&[u8]>>, tag: Tag| {
        decoder
            .find_tag_unsigned::<u16>(tag)
            .map_err(tiff_error)
            .map(|found| found.unwrap_or(0))
    };
    let compression = unsigned(decoder, Tag::Compression)?;
    let predictor = unsigned(decoder, Tag::Predictor)?;
    let photometric = decoder
        .find_tag_unsigned::<u16>(Tag::PhotometricInterpretation)
        .map_err(tiff_error)?
        .unwrap_or(u16::MAX);
    let planar = unsigned(decoder, Tag::PlanarConfiguration)?;
    let depths = decoder
        .find_tag_unsigned_vec::<u16>(Tag::BitsPerSample)
        .map_err(tiff_error)?
        .unwrap_or_default();
    let formats = decoder
        .find_tag_unsigned_vec::<u16>(Tag::SampleFormat)
        .map_err(tiff_error)?
        .unwrap_or_default();
    let tag = decoder
        .find_tag_unsigned::<u16>(Tag::Orientation)
        .map_err(tiff_error)?
        .and_then(ExifOrientation::from_tag)
        .unwrap_or(ExifOrientation::UPRIGHT);
    let tags: Vec<u16> = decoder
        .tag_iter()
        .filter_map(Result::ok)
        .map(|(tag, _)| tag.to_u16())
        .collect();
    let depth = depths.first().copied().unwrap_or(0);
    let samples = depths.len();
    let supported = matches!(compression, 1 | 5 | 8 | 32946 | 32773)
        && matches!(predictor, 0..=2)
        && matches!(photometric, 0..=3)
        && matches!(planar, 0 | 1)
        && matches!(depth, 8 | 16)
        && depths.iter().all(|d| *d == depth)
        && formats.iter().all(|f| *f == 1)
        && (1..=4).contains(&samples)
        && (photometric != 3 || samples == 1);
    Ok(Facts {
        compression,
        predictor: if predictor == 2 { 2 } else { 1 },
        photometric,
        depth,
        samples,
        orientation: tag,
        points_elsewhere: tags.iter().any(|t| POINTERS.contains(t)),
        supported,
    })
}

/// The facts of every page, or `None` when the file cannot be read as a TIFF.
fn pages(file: &[u8]) -> Option<Vec<Facts>> {
    let mut decoder = Decoder::new(Cursor::new(file)).ok()?;
    let mut found = Vec::new();
    loop {
        found.push(facts_of(&mut decoder).ok()?);
        if !decoder.more_images() {
            return Some(found);
        }
        if found.len() >= MAX_PAGES {
            return None;
        }
        decoder.next_image().ok()?;
    }
}

/// What turning the file costs. Nothing when each page is a plain 8 or 16-bit picture the
/// encoder writes; the extra directories a multi-page file points to when there are any; and
/// when a page cannot be written as it is, every page but the first, or colour detail.
pub(super) fn fidelity(file: &[u8]) -> Fidelity {
    let Some(pages) = pages(file).filter(|p| !p.is_empty()) else {
        return Fidelity::Impossible;
    };
    let pointed = pages.iter().any(|p| p.points_elsewhere);
    if !pages.iter().all(|p| p.supported) {
        Fidelity::Loses(if pages.len() > 1 {
            Loss::Pages
        } else {
            Loss::Colour
        })
    } else if !pointed
        || (pages.len() == 1 && with_orientation(file, ExifOrientation::UPRIGHT).is_ok())
    {
        Fidelity::Intact
    } else {
        Fidelity::Loses(Loss::Details)
    }
}

/// The sample bytes and shape of a page that carries its samples as `u8` or `u16`.
enum Samples {
    Narrow(Vec<u8>),
    Wide(Vec<u16>),
}

/// A sample type's number of channels as a colour type for the encoder: the photometric
/// interpretation and every other tag are written over what it says.
struct Interleaved8<const N: usize>;
struct Interleaved16<const N: usize>;

impl<const N: usize> ColorType for Interleaved8<N> {
    type Inner = u8;
    const TIFF_VALUE: tiff::tags::PhotometricInterpretation =
        tiff::tags::PhotometricInterpretation::BlackIsZero;
    const BITS_PER_SAMPLE: &'static [u16] = &[8; N];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::Uint; N];

    fn horizontal_predict(row: &[u8], result: &mut Vec<u8>) {
        predicted(row, result, N, u8::wrapping_sub);
    }
}

impl<const N: usize> ColorType for Interleaved16<N> {
    type Inner = u16;
    const TIFF_VALUE: tiff::tags::PhotometricInterpretation =
        tiff::tags::PhotometricInterpretation::BlackIsZero;
    const BITS_PER_SAMPLE: &'static [u16] = &[16; N];
    const SAMPLE_FORMAT: &'static [SampleFormat] = &[SampleFormat::Uint; N];

    fn horizontal_predict(row: &[u16], result: &mut Vec<u16>) {
        predicted(row, result, N, u16::wrapping_sub);
    }
}

/// `row` with each sample replaced by its difference from the one a pixel before.
fn predicted<T: Copy>(row: &[T], result: &mut Vec<T>, stride: usize, minus: fn(T, T) -> T) {
    if row.len() < stride {
        return;
    }
    let (first, rest) = row.split_at(stride);
    result.extend_from_slice(first);
    result.extend(
        row.iter()
            .zip(rest)
            .map(|(before, now)| minus(*now, *before)),
    );
}

/// Reads the pixels of the current page as one byte-addressed grid, with a white-is-zero page
/// turned back to the values the file stores.
fn page_grid(decoder: &mut Decoder<Cursor<&[u8]>>, facts: &Facts) -> Result<Grid, ImageError> {
    let (width, height) = decoder.dimensions().map_err(tiff_error)?;
    let bytes = match decoder.read_image().map_err(tiff_error)? {
        DecodingResult::U8(v) => v,
        DecodingResult::U16(v) => v.iter().flat_map(|s| s.to_ne_bytes()).collect(),
        DecodingResult::U32(_)
        | DecodingResult::U64(_)
        | DecodingResult::F16(_)
        | DecodingResult::F32(_)
        | DecodingResult::F64(_)
        | DecodingResult::I8(_)
        | DecodingResult::I16(_)
        | DecodingResult::I32(_)
        | DecodingResult::I64(_) => {
            return Err(ImageError::Container {
                reason: "a sample type this writer does not keep".to_owned(),
            });
        }
    };
    let mut grid = Grid {
        width,
        height,
        unit: facts.samples * usize::from(facts.depth / 8),
        bytes,
    };
    if facts.photometric == 0 {
        // The decoder shows white-is-zero data inverted; the file keeps it as it was.
        grid.bytes.iter_mut().for_each(|b| *b = !*b);
    }
    Ok(grid)
}

fn samples_of(grid: &Grid, facts: &Facts) -> Samples {
    if facts.depth == 8 {
        Samples::Narrow(grid.bytes.clone())
    } else {
        Samples::Wide(
            grid.bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u16::from_ne_bytes(*pair))
                .collect(),
        )
    }
}

/// Every tag of the current page that survives the move, with the resolution's axes swapped when
/// the page was turned a quarter.
fn carried_tags(
    decoder: &mut Decoder<Cursor<&[u8]>>,
    swap: bool,
) -> Result<Vec<(Tag, ValueBuffer)>, ImageError> {
    let tags: Vec<Tag> = decoder
        .tag_iter()
        .filter_map(Result::ok)
        .map(|(tag, _)| tag)
        .filter(|tag| !STRUCTURAL.contains(&tag.to_u16()) && !POINTERS.contains(&tag.to_u16()))
        .collect();
    let mut carried = Vec::new();
    for tag in tags {
        let mut buffer = ValueBuffer::empty(tiff::tags::Type::BYTE);
        let found = decoder
            .image_ifd()
            .find_tag_buf(tag, &mut buffer)
            .map_err(tiff_error)?;
        if found.is_none() {
            continue;
        }
        buffer.set_byte_order(ByteOrder::native());
        let tag = match (swap, tag.to_u16()) {
            (true, X_RESOLUTION) => Tag::from_u16_exhaustive(Y_RESOLUTION),
            (true, Y_RESOLUTION) => Tag::from_u16_exhaustive(X_RESOLUTION),
            _ => tag,
        };
        carried.push((tag, buffer));
    }
    Ok(carried)
}

fn compression_of(first: &Facts) -> Compression {
    match first.compression {
        5 => Compression::Lzw,
        8 | 32946 => Compression::Deflate(DeflateLevel::Balanced),
        32773 => Compression::Packbits,
        _ => Compression::Uncompressed,
    }
}

/// Writes one page: its samples, and the tags over the encoder's own.
fn write_page<W: Write + Seek>(
    encoder: &mut TiffEncoder<W>,
    grid: &Grid,
    facts: &Facts,
    tags: &[(Tag, ValueBuffer)],
) -> Result<(), ImageError> {
    let samples = samples_of(grid, facts);
    macro_rules! page {
        ($colour:ty, $data:expr) => {{
            let mut image = encoder
                .new_image::<$colour>(grid.width, grid.height)
                .map_err(tiff_error)?;
            for (tag, value) in tags {
                image
                    .encoder()
                    .write_tag_buf(*tag, value)
                    .map_err(tiff_error)?;
            }
            image.write_data($data).map_err(tiff_error)
        }};
    }
    match (samples, facts.samples) {
        (Samples::Narrow(d), 1) => page!(Interleaved8<1>, &d),
        (Samples::Narrow(d), 2) => page!(Interleaved8<2>, &d),
        (Samples::Narrow(d), 3) => page!(Interleaved8<3>, &d),
        (Samples::Narrow(d), 4) => page!(Interleaved8<4>, &d),
        (Samples::Wide(d), 1) => page!(Interleaved16<1>, &d),
        (Samples::Wide(d), 2) => page!(Interleaved16<2>, &d),
        (Samples::Wide(d), 3) => page!(Interleaved16<3>, &d),
        (Samples::Wide(d), 4) => page!(Interleaved16<4>, &d),
        _ => Err(ImageError::Container {
            reason: "a sample layout this writer does not keep".to_owned(),
        }),
    }
}

/// `file` with `placing` applied to every page, everything else as it was.
pub(super) fn rewritten(file: &[u8], placing: Placing) -> Result<Vec<u8>, ImageError> {
    let all = pages(file).ok_or_else(|| ImageError::Container {
        reason: "not a TIFF".to_owned(),
    })?;
    if all.len() == 1 && all.iter().any(|p| p.points_elsewhere) {
        let target = placing.over(ExifFacts::read(file).orientation);
        if let Ok(turned) = with_orientation(file, target) {
            return Ok(turned);
        }
    }
    let first = all.first().ok_or_else(|| ImageError::Container {
        reason: "a TIFF with no pages".to_owned(),
    })?;
    let mut decoder = Decoder::new(Cursor::new(file)).map_err(tiff_error)?;
    let mut out = Cursor::new(Vec::new());
    let mut encoder = TiffEncoder::new(&mut out)
        .map_err(tiff_error)?
        .with_compression(compression_of(first))
        .with_predictor(if first.predictor == 2 {
            Predictor::Horizontal
        } else {
            Predictor::None
        });
    for (index, facts) in all.iter().enumerate() {
        if index > 0 {
            decoder.next_image().map_err(tiff_error)?;
        }
        let total = placing.over(facts.orientation);
        let tags = carried_tags(&mut decoder, swaps_axes(total))?;
        let grid = page_grid(&mut decoder, facts)?.placed(total);
        write_page(&mut encoder, &grid, facts, &tags)?;
    }
    let _ = encoder;
    Ok(out.into_inner())
}
