//! Turning and flipping keep what a person would notice: depth, colour type, palette, pages,
//! text, resolution and colour profile. Every file is built here from scratch.

// Helpers in an integration test crate are not `#[test]` functions, so clippy.toml does not cover them.
#![allow(clippy::unwrap_used)]

mod support;

use anyview_core::{Axis, Edit, QuarterTurn};
use anyview_image::{Fidelity, Loss, edited, fidelity};
use image::codecs::bmp::BmpEncoder;
use image::codecs::webp::WebPEncoder;
use image::{ExtendedColorType, ImageEncoder};
use std::io::Cursor;
use support::{bytes, sniffed};

const TURN: Edit = Edit::Rotate(QuarterTurn::Quarter);

fn four_turns(file: &[u8], name: &str) -> Vec<u8> {
    let mut now = file.to_vec();
    for _ in 0..4 {
        now = edited(&now, &sniffed(&now, name), TURN).unwrap();
    }
    now
}

/// The chunks of a PNG as (type, data), without the picture data.
fn png_chunks(file: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut at = 8;
    let mut out = Vec::new();
    while at < file.len() {
        let len = u32::from_be_bytes(file[at..at + 4].try_into().unwrap()) as usize;
        let kind = String::from_utf8_lossy(&file[at + 4..at + 8]).into_owned();
        out.push((kind, file[at + 8..at + 8 + len].to_vec()));
        at += 12 + len;
    }
    out
}

fn named<'a>(chunks: &'a [(String, Vec<u8>)], kind: &str) -> Option<&'a [u8]> {
    chunks
        .iter()
        .find(|(k, _)| k == kind)
        .map(|(_, d)| d.as_slice())
}

/// Pixel data of a PNG exactly as stored (no expansion), and its header facts.
fn png_raw(file: &[u8]) -> (png::Info<'static>, Vec<u8>) {
    let mut decoder = png::Decoder::new(Cursor::new(file));
    decoder.set_transformations(png::Transformations::IDENTITY);
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let frame = reader.next_frame(&mut buf).unwrap();
    buf.truncate(frame.buffer_size());
    (reader.info().clone(), buf)
}

fn grey16_png() -> Vec<u8> {
    let (w, h) = (5u32, 3u32);
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, w, h);
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Sixteen);
    encoder.set_pixel_dims(Some(png::PixelDimensions {
        xppu: 11811,
        yppu: 5906,
        unit: png::Unit::Meter,
    }));
    encoder.set_source_gamma(png::ScaledFloat::new(0.45455));
    encoder
        .add_text_chunk("Comment".to_owned(), "kept".to_owned())
        .unwrap();
    encoder
        .add_itxt_chunk("XML:com.adobe.xmp".to_owned(), "<x:xmpmeta/>".to_owned())
        .unwrap();
    let mut writer = encoder.write_header().unwrap();
    let data: Vec<u8> = (0..w * h)
        .flat_map(|i| (i as u16 * 4001 + 17).to_be_bytes())
        .collect();
    writer.write_image_data(&data).unwrap();
    writer.finish().unwrap();
    out
}

#[test]
fn a_sixteen_bit_grey_png_stays_sixteen_bit_grey_with_its_chunks() {
    let before = grey16_png();
    let after = edited(&before, &sniffed(&before, "a.png"), TURN).unwrap();
    let (info, _) = png_raw(&after);
    assert_eq!(info.color_type, png::ColorType::Grayscale);
    assert_eq!(info.bit_depth, png::BitDepth::Sixteen);
    assert_eq!((info.width, info.height), (3, 5));
    let chunks = png_chunks(&after);
    for kind in ["tEXt", "iTXt", "gAMA", "pHYs"] {
        assert!(named(&chunks, kind).is_some(), "{kind} is kept");
    }
    let phys = named(&chunks, "pHYs").unwrap();
    assert_eq!(u32::from_be_bytes(phys[0..4].try_into().unwrap()), 5906);
    assert_eq!(u32::from_be_bytes(phys[4..8].try_into().unwrap()), 11811);
    assert_eq!(four_turns(&before, "a.png").len() > 0, true);
    assert_eq!(png_raw(&four_turns(&before, "a.png")).1, png_raw(&before).1);
}

fn palette_png(depth: png::BitDepth) -> Vec<u8> {
    let (w, h) = (7u32, 4u32);
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, w, h);
    encoder.set_color(png::ColorType::Indexed);
    encoder.set_depth(depth);
    encoder.set_palette(vec![10, 20, 30, 200, 100, 50, 0, 255, 0, 9, 9, 9]);
    encoder.set_trns(vec![255, 128]);
    encoder
        .add_text_chunk("Comment".to_owned(), "a note".to_owned())
        .unwrap();
    encoder
        .add_ztxt_chunk("Software".to_owned(), "tests".to_owned())
        .unwrap();
    let mut writer = encoder.write_header().unwrap();
    let bits = match depth {
        png::BitDepth::One => 1,
        png::BitDepth::Two => 2,
        png::BitDepth::Four => 4,
        _ => 8,
    };
    let line = (w as usize * bits).div_ceil(8);
    let mut data = vec![0u8; line * h as usize];
    for y in 0..h as usize {
        for x in 0..w as usize {
            let index = ((x + y * 3) % 4) as u8;
            let bit = x * bits;
            data[y * line + bit / 8] |= index << (8 - bits - bit % 8);
        }
    }
    writer.write_image_data(&data).unwrap();
    writer.finish().unwrap();
    out
}

#[test]
fn a_palette_png_keeps_its_palette_transparency_and_text() {
    for depth in [
        png::BitDepth::Two,
        png::BitDepth::Four,
        png::BitDepth::Eight,
    ] {
        let before = palette_png(depth);
        for edit in [TURN, Edit::Flip(Axis::Vertical)] {
            let after = edited(&before, &sniffed(&before, "a.png"), edit).unwrap();
            let (info, _) = png_raw(&after);
            assert_eq!(info.color_type, png::ColorType::Indexed, "{depth:?}");
            assert_eq!(info.bit_depth, depth);
            let (was, now) = (png_chunks(&before), png_chunks(&after));
            for kind in ["PLTE", "tRNS", "tEXt", "zTXt"] {
                assert_eq!(named(&was, kind), named(&now, kind), "{kind} at {depth:?}");
            }
        }
        let (_, raw_before) = png_raw(&before);
        let (_, raw_after) = png_raw(&four_turns(&before, "a.png"));
        assert_eq!(raw_before, raw_after, "four turns at {depth:?}");
    }
}

fn tiff_pages(white_is_zero: bool) -> Vec<u8> {
    use tiff::encoder::colortype::{Gray8, RGB16};
    use tiff::encoder::{Compression, Rational, TiffEncoder};
    use tiff::tags::{ResolutionUnit, Tag};
    let mut out = Cursor::new(Vec::new());
    let mut encoder = TiffEncoder::new(&mut out)
        .unwrap()
        .with_compression(Compression::Lzw);
    for (n, (w, h)) in [(4u32, 3u32), (3, 5), (6, 2)].into_iter().enumerate() {
        if white_is_zero {
            let mut image = encoder.new_image::<Gray8>(w, h).unwrap();
            image
                .encoder()
                .write_tag(Tag::PhotometricInterpretation, 0u16)
                .unwrap();
            let data: Vec<u8> = (0..w * h).map(|i| (i * 9 + n as u32) as u8).collect();
            image.write_data(&data).unwrap();
        } else {
            let mut image = encoder.new_image::<RGB16>(w, h).unwrap();
            image.resolution(ResolutionUnit::Inch, Rational { n: 300, d: 1 });
            image.y_resolution(Rational { n: 150, d: 1 });
            image
                .encoder()
                .write_tag(Tag::ImageDescription, &format!("page {n}")[..])
                .unwrap();
            let data: Vec<u16> = (0..w * h * 3)
                .map(|i| (i * 1021 + n as u32 * 7) as u16)
                .collect();
            image.write_data(&data).unwrap();
        }
    }
    drop(encoder);
    out.into_inner()
}

struct Page {
    size: (u32, u32),
    description: Option<String>,
    resolution: (u32, u32),
    compression: u16,
    bits: Vec<u16>,
    photometric: u16,
    pixels: tiff::decoder::DecodingResult,
}

fn pages_of(file: &[u8]) -> Vec<Page> {
    use tiff::tags::Tag;
    let mut decoder = tiff::decoder::Decoder::new(Cursor::new(file)).unwrap();
    let mut pages = Vec::new();
    loop {
        let rational =
            |d: &mut tiff::decoder::Decoder<Cursor<&[u8]>>, tag| match d.find_tag(tag).unwrap() {
                Some(tiff::decoder::ifd::Value::Rational(n, _)) => n,
                _ => 0,
            };
        pages.push(Page {
            size: decoder.dimensions().unwrap(),
            description: decoder.get_tag_ascii_string(Tag::ImageDescription).ok(),
            resolution: (
                rational(&mut decoder, Tag::XResolution),
                rational(&mut decoder, Tag::YResolution),
            ),
            compression: decoder.get_tag_u32(Tag::Compression).unwrap() as u16,
            bits: decoder.get_tag_u16_vec(Tag::BitsPerSample).unwrap(),
            photometric: decoder.get_tag_u32(Tag::PhotometricInterpretation).unwrap() as u16,
            pixels: decoder.read_image().unwrap(),
        });
        if !decoder.more_images() {
            return pages;
        }
        decoder.next_image().unwrap();
    }
}

fn same_samples(a: &tiff::decoder::DecodingResult, b: &tiff::decoder::DecodingResult) -> bool {
    use tiff::decoder::DecodingResult::{U8, U16};
    match (a, b) {
        (U8(x), U8(y)) => x == y,
        (U16(x), U16(y)) => x == y,
        _ => false,
    }
}

#[test]
fn a_three_page_tiff_keeps_every_page_depth_and_tag() {
    let before = tiff_pages(false);
    let after = edited(&before, &sniffed(&before, "a.tif"), TURN).unwrap();
    let (was, now) = (pages_of(&before), pages_of(&after));
    assert_eq!(now.len(), 3, "no page is dropped");
    for (n, (a, b)) in was.iter().zip(&now).enumerate() {
        assert_eq!(b.size, (a.size.1, a.size.0), "page {n} turned");
        assert_eq!(b.bits, vec![16, 16, 16], "page {n} depth");
        assert_eq!(b.compression, a.compression, "page {n} LZW");
        assert_eq!(b.photometric, 2);
        assert_eq!(b.description, a.description);
        assert_eq!(b.resolution, (a.resolution.1, a.resolution.0), "axes swap");
    }
    let round = pages_of(&four_turns(&before, "a.tif"));
    for (a, b) in was.iter().zip(&round) {
        assert_eq!(a.size, b.size);
        assert!(
            same_samples(&a.pixels, &b.pixels),
            "four turns are identical"
        );
    }
}

#[test]
fn a_white_is_zero_tiff_stays_white_is_zero() {
    let before = tiff_pages(true);
    let after = edited(
        &before,
        &sniffed(&before, "a.tif"),
        Edit::Flip(Axis::Horizontal),
    )
    .unwrap();
    assert!(pages_of(&after).iter().all(|p| p.photometric == 0));
    let twice = edited(
        &after,
        &sniffed(&after, "a.tif"),
        Edit::Flip(Axis::Horizontal),
    )
    .unwrap();
    for (a, b) in pages_of(&before).iter().zip(&pages_of(&twice)) {
        assert!(same_samples(&a.pixels, &b.pixels));
    }
}

fn riff(chunks: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
    let mut body = b"WEBP".to_vec();
    for (code, data) in chunks {
        body.extend_from_slice(*code);
        body.extend_from_slice(&(data.len() as u32).to_le_bytes());
        body.extend_from_slice(data);
        if data.len() % 2 == 1 {
            body.push(0);
        }
    }
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend(body);
    out
}

fn riff_chunks(file: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut at = 12;
    let mut out = Vec::new();
    while at + 8 <= file.len() {
        let size = u32::from_le_bytes(file[at + 4..at + 8].try_into().unwrap()) as usize;
        out.push((
            String::from_utf8_lossy(&file[at..at + 4]).into_owned(),
            file[at + 8..at + 8 + size].to_vec(),
        ));
        at += 8 + size + (size & 1);
    }
    out
}

fn lossless_webp_with_metadata() -> Vec<u8> {
    let (w, h) = (6u32, 4u32);
    let pixels: Vec<u8> = (0..w * h * 4).map(|i| (i * 7) as u8).collect();
    let mut plain = Vec::new();
    WebPEncoder::new_lossless(&mut plain)
        .write_image(&pixels, w, h, ExtendedColorType::Rgba8)
        .unwrap();
    let vp8l = riff_chunks(&plain)
        .into_iter()
        .find(|(code, _)| code == "VP8L")
        .unwrap()
        .1;
    let mut vp8x = vec![0x20 | 0x10 | 0x04, 0, 0, 0];
    vp8x.extend_from_slice(&(w - 1).to_le_bytes()[..3]);
    vp8x.extend_from_slice(&(h - 1).to_le_bytes()[..3]);
    riff(&[
        (b"VP8X", vp8x),
        (b"ICCP", b"profile-bytes".to_vec()),
        (b"VP8L", vp8l),
        (b"XMP ", b"<x:xmpmeta>hi</x:xmpmeta>".to_vec()),
    ])
}

#[test]
fn a_lossless_webp_stays_lossless_and_keeps_its_profile_and_xmp() {
    let before = lossless_webp_with_metadata();
    let after = edited(&before, &sniffed(&before, "a.webp"), TURN).unwrap();
    let chunks = riff_chunks(&after);
    let find = |code: &str| {
        chunks
            .iter()
            .find(|(c, _)| c == code)
            .map(|(_, d)| d.clone())
    };
    assert!(
        find("VP8L").is_some() && find("VP8 ").is_none(),
        "still lossless"
    );
    assert_eq!(find("ICCP").as_deref(), Some(&b"profile-bytes"[..]));
    assert_eq!(
        find("XMP ").as_deref(),
        Some(&b"<x:xmpmeta>hi</x:xmpmeta>"[..])
    );
    let first = image::load_from_memory(&before).unwrap().to_rgba8();
    let last = image::load_from_memory(&four_turns(&before, "a.webp"))
        .unwrap()
        .to_rgba8();
    assert_eq!(first.as_raw(), last.as_raw(), "four turns are identical");
    let once = image::load_from_memory(&after).unwrap();
    assert_eq!((once.width(), once.height()), (4, 6));
}

#[test]
fn a_lossy_webp_is_written_without_asking_and_an_animated_one_asks() {
    let lossy = bytes("lossy.webp");
    assert_eq!(
        fidelity(&lossy, &sniffed(&lossy, "a.webp")),
        Fidelity::Intact
    );
    assert!(edited(&lossy, &sniffed(&lossy, "a.webp"), TURN).is_ok());
    let moving = bytes("anim.webp");
    assert_eq!(
        fidelity(&moving, &sniffed(&moving, "a.webp")),
        Fidelity::Loses(Loss::Animation)
    );
    assert!(edited(&moving, &sniffed(&moving, "a.webp"), TURN).is_ok());
}

#[test]
fn a_bmp_keeps_its_depth_palette_and_resolution() {
    let (w, h) = (5u32, 3u32);
    let mut paletted = Vec::new();
    let palette: Vec<[u8; 3]> = (0..256).map(|i| [i as u8, 255 - i as u8, 7]).collect();
    let indices: Vec<u8> = (0..w * h).map(|i| (i * 17) as u8).collect();
    BmpEncoder::new(&mut paletted)
        .encode_with_palette(&indices, w, h, ExtendedColorType::L8, Some(&palette))
        .unwrap();
    let mut rgb = Vec::new();
    let colour: Vec<u8> = (0..w * h * 3).map(|i| (i * 5) as u8).collect();
    BmpEncoder::new(&mut rgb)
        .encode(&colour, w, h, ExtendedColorType::Rgb8)
        .unwrap();
    for (name, file, bits) in [("8-bit", paletted, 8u16), ("24-bit", rgb, 24)] {
        let mut file = file;
        file[38..42].copy_from_slice(&2835u32.to_le_bytes());
        file[42..46].copy_from_slice(&5669u32.to_le_bytes());
        let after = edited(&file, &sniffed(&file, "a.bmp"), TURN).unwrap();
        assert_eq!(
            u16::from_le_bytes(after[28..30].try_into().unwrap()),
            bits,
            "{name}"
        );
        assert_eq!(
            u32::from_le_bytes(after[18..22].try_into().unwrap()),
            3,
            "{name}"
        );
        assert_eq!(
            u32::from_le_bytes(after[38..42].try_into().unwrap()),
            5669,
            "{name}"
        );
        let round = four_turns(&file, "a.bmp");
        let decode = |f: &[u8]| image::load_from_memory(f).unwrap().to_rgba8().into_raw();
        assert_eq!(decode(&file), decode(&round), "{name} four turns");
        if bits == 8 {
            assert_eq!(after[54..54 + 1024], file[54..54 + 1024], "palette kept");
        }
    }
}

#[test]
fn what_cannot_be_kept_asks_and_what_cannot_be_written_is_hidden() {
    use tiff::encoder::TiffEncoder;
    use tiff::encoder::colortype::CMYK8;
    let mut cmyk = Cursor::new(Vec::new());
    TiffEncoder::new(&mut cmyk)
        .unwrap()
        .write_image::<CMYK8>(2, 2, &[0u8; 16])
        .unwrap();
    let cmyk = cmyk.into_inner();
    let mut animated = Vec::new();
    let mut encoder = png::Encoder::new(&mut animated, 2, 2);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_animated(2, 0).unwrap();
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&[0u8; 16]).unwrap();
    writer.write_image_data(&[255u8; 16]).unwrap();
    writer.finish().unwrap();
    let gif = bytes("spin.gif");
    let cases: Vec<(&str, &[u8], &str, Fidelity)> = vec![
        ("a CMYK tiff", &cmyk, "a.tif", Fidelity::Loses(Loss::Colour)),
        (
            "an animated png",
            &animated,
            "a.png",
            Fidelity::Loses(Loss::Animation),
        ),
        ("an animated gif", &gif, "spin.gif", Fidelity::Impossible),
    ];
    for (name, file, called, want) in cases {
        assert_eq!(fidelity(file, &sniffed(file, called)), want, "{name}");
        let written = edited(file, &sniffed(file, called), TURN);
        assert_eq!(written.is_ok(), want != Fidelity::Impossible, "{name}");
    }
    let plain = grey16_png();
    assert_eq!(
        fidelity(&plain, &sniffed(&plain, "a.png")),
        Fidelity::Intact
    );
}
