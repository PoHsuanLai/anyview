//! The freedesktop thumbnail cache: `<cache>/thumbnails/<size>/<md5 of the file URI>.png`, a PNG
//! whose `Thumb::URI` and `Thumb::MTime` text chunks say which file, and which version of it,
//! the picture was made from. A thumbnail whose chunks disagree with the file is stale, so
//! there is none.

use crate::env::Env;
use crate::error::{IoOp, PlatformError};
use crate::thumbnail::{ThumbPixels, ThumbSize, ThumbnailCache};
use crate::uri::file_uri;
use anyview_core::{FilePath, FileStamp, PixelLen, PixelSize};
use md5::{Digest, Md5};
use std::fs::{DirBuilder, File, OpenOptions};
use std::io::{BufWriter, Cursor, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

const URI_KEY: &str = "Thumb::URI";
const MTIME_KEY: &str = "Thumb::MTime";
const SIZE_KEY: &str = "Thumb::Size";
/// The spec asks for a private cache: only the owner reads it.
const PRIVATE_FILE: u32 = 0o600;
const PRIVATE_DIR: u32 = 0o700;

/// [`ThumbnailCache`] under `env`'s cache directory.
#[derive(Debug, Clone)]
pub struct FreedesktopThumbnails {
    root: PathBuf,
}

impl FreedesktopThumbnails {
    /// The cache at `<cache>/thumbnails`.
    pub fn new(env: &Env) -> Self {
        FreedesktopThumbnails {
            root: env.dirs.cache.join("thumbnails"),
        }
    }

    /// Where the thumbnail of `file` at `size` lives.
    fn path(&self, file: &FilePath, size: ThumbSize) -> PathBuf {
        let digest = Md5::digest(file_uri(file).as_bytes());
        let name: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        self.root.join(size.directory()).join(format!("{name}.png"))
    }
}

/// The modification time as the spec's whole seconds since the epoch.
fn mtime_seconds(stamp: &FileStamp) -> i64 {
    stamp.modified.0.div_euclid(1_000_000_000)
}

impl ThumbnailCache for FreedesktopThumbnails {
    fn lookup(
        &self,
        file: &FilePath,
        stamp: &FileStamp,
        size: ThumbSize,
    ) -> Result<Option<ThumbPixels>, PlatformError> {
        let path = self.path(file, size);
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(PlatformError::io(IoOp::Read, path, &error)),
        };
        Ok(decode(&bytes, &file_uri(file), mtime_seconds(stamp)))
    }

    fn store(
        &self,
        file: &FilePath,
        stamp: &FileStamp,
        size: ThumbSize,
        pixels: &ThumbPixels,
    ) -> Result<(), PlatformError> {
        let path = self.path(file, size);
        let directory = path.parent().unwrap_or(&self.root);
        DirBuilder::new()
            .recursive(true)
            .mode(PRIVATE_DIR)
            .create(directory)
            .map_err(|error| PlatformError::io(IoOp::CreateDir, directory, &error))?;
        let tags = Tags {
            uri: file_uri(file),
            mtime: mtime_seconds(stamp),
            len: stamp.len.0,
        };
        let png = encode(pixels, &tags).map_err(|error| PlatformError::Thumbnail {
            path: path.clone(),
            reason: error.to_string(),
        })?;
        write_atomically(&path, &png)
    }
}

/// The text chunks a thumbnail carries.
struct Tags {
    uri: String,
    mtime: i64,
    len: u64,
}

/// `pixels` as a PNG with the spec's text chunks before the image data.
fn encode(pixels: &ThumbPixels, tags: &Tags) -> Result<Vec<u8>, png::EncodingError> {
    let size = pixels.size();
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, size.width.0, size.height.0);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    for (key, value) in [
        (URI_KEY, tags.uri.clone()),
        (MTIME_KEY, tags.mtime.to_string()),
        (SIZE_KEY, tags.len.to_string()),
    ] {
        encoder.add_text_chunk(key.to_owned(), value)?;
    }
    let mut writer = encoder.write_header()?;
    writer.write_image_data(pixels.rgba())?;
    writer.finish()?;
    Ok(out)
}

/// The pixels of a thumbnail made for `uri` at `mtime`; `None` when the bytes are not a PNG,
/// name another file or version, or are not 8-bit colour.
fn decode(bytes: &[u8], uri: &str, mtime: i64) -> Option<ThumbPixels> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let text = |key: &str| {
        reader
            .info()
            .uncompressed_latin1_text
            .iter()
            .find(|chunk| chunk.keyword == key)
            .map(|chunk| chunk.text.clone())
    };
    let same_file = text(URI_KEY).is_some_and(|text| text == uri);
    let same_version = text(MTIME_KEY).and_then(|text| text.parse::<i64>().ok()) == Some(mtime);
    if !(same_file && same_version) {
        return None;
    }
    let mut frame = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut frame).ok()?;
    frame.truncate(info.buffer_size());
    let rgba = to_rgba(&frame, info.color_type)?;
    ThumbPixels::new(
        PixelSize {
            width: PixelLen(info.width),
            height: PixelLen(info.height),
        },
        rgba,
    )
}

/// 8-bit samples of `color` as RGBA; `None` for a palette type, which `EXPAND` has already
/// removed.
fn to_rgba(samples: &[u8], color: png::ColorType) -> Option<Vec<u8>> {
    match color {
        png::ColorType::Rgba => Some(samples.to_vec()),
        png::ColorType::Rgb => Some(
            samples
                .as_chunks::<3>()
                .0
                .iter()
                .flat_map(|px| [px[0], px[1], px[2], 255])
                .collect(),
        ),
        png::ColorType::Grayscale => Some(
            samples
                .iter()
                .flat_map(|&grey| [grey, grey, grey, 255])
                .collect(),
        ),
        png::ColorType::GrayscaleAlpha => Some(
            samples
                .as_chunks::<2>()
                .0
                .iter()
                .flat_map(|px| [px[0], px[0], px[0], px[1]])
                .collect(),
        ),
        png::ColorType::Indexed => None,
    }
}

/// Write `bytes` beside `path` and rename them over it, so a reader never sees half a file.
fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), PlatformError> {
    let temporary = path.with_extension("png.tmp");
    let write = || -> std::io::Result<()> {
        let file: File = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(PRIVATE_FILE)
            .open(&temporary)?;
        let mut out = BufWriter::new(file);
        out.write_all(bytes)?;
        out.into_inner().map_err(|e| e.into_error())?.sync_all()
    };
    write().map_err(|error| PlatformError::io(IoOp::Write, &temporary, &error))?;
    std::fs::rename(&temporary, path).map_err(|error| PlatformError::io(IoOp::Rename, path, &error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::{ByteLen, ModTime};

    fn file(path: &str) -> FilePath {
        FilePath::new(path).unwrap()
    }

    fn stamp(seconds: i64) -> FileStamp {
        FileStamp {
            len: ByteLen(10),
            modified: ModTime(seconds * 1_000_000_000 + 7),
        }
    }

    fn pixels(width: u32) -> ThumbPixels {
        let rgba = (0..width * 2 * 4).map(|i| i as u8).collect();
        let size = PixelSize {
            width: PixelLen(width),
            height: PixelLen(2),
        };
        ThumbPixels::new(size, rgba).unwrap()
    }

    fn cache(scratch: &tempfile::TempDir) -> FreedesktopThumbnails {
        FreedesktopThumbnails::new(&Env::isolated(scratch.path()))
    }

    #[test]
    fn the_name_is_the_md5_of_the_uri_in_the_size_directory() {
        // The hashes come from `md5sum` over the URI text, not from this crate.
        const CASES: &[(&str, &str, ThumbSize, &str)] = &[
            (
                "normal",
                "/home/jens/photos/me.png",
                ThumbSize::Normal,
                "normal/c6ee772d9e49320e97ec29a7eb5b1697.png",
            ),
            (
                "large and escaped",
                "/tmp/a b.png",
                ThumbSize::Large,
                "large/f2584ab78dd95a88bd0d3f0ecaee7a8c.png",
            ),
            (
                "x-large",
                "/tmp/a b.png",
                ThumbSize::XLarge,
                "x-large/f2584ab78dd95a88bd0d3f0ecaee7a8c.png",
            ),
        ];
        let scratch = tempfile::tempdir().unwrap();
        let cache = cache(&scratch);
        for (name, path, size, relative) in CASES {
            let want = scratch.path().join("cache/thumbnails").join(relative);
            assert_eq!(cache.path(&file(path), *size), want, "{name}");
        }
    }

    #[test]
    fn a_stored_thumbnail_comes_back_for_the_same_version_only() {
        let scratch = tempfile::tempdir().unwrap();
        let cache = cache(&scratch);
        let path = file("/pics/cat.png");
        let held = pixels(3);
        cache
            .store(&path, &stamp(100), ThumbSize::Normal, &held)
            .unwrap();
        let found = cache.lookup(&path, &stamp(100), ThumbSize::Normal).unwrap();
        assert_eq!(found, Some(held));
        const MISSES: &[(&str, &str, i64, ThumbSize)] = &[
            ("edited", "/pics/cat.png", 101, ThumbSize::Normal),
            ("other file", "/pics/dog.png", 100, ThumbSize::Normal),
            ("other size", "/pics/cat.png", 100, ThumbSize::Large),
        ];
        for (name, path, seconds, size) in MISSES {
            let found = cache.lookup(&file(path), &stamp(*seconds), *size).unwrap();
            assert_eq!(found, None, "{name}");
        }
    }

    #[test]
    fn the_file_carries_the_spec_chunks_and_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = tempfile::tempdir().unwrap();
        let cache = cache(&scratch);
        let path = file("/pics/a b.png");
        cache
            .store(&path, &stamp(5), ThumbSize::Large, &pixels(2))
            .unwrap();
        let written = cache.path(&path, ThumbSize::Large);
        let bytes = std::fs::read(&written).unwrap();
        let reader = png::Decoder::new(Cursor::new(&bytes)).read_info().unwrap();
        let chunks: Vec<(String, String)> = reader
            .info()
            .uncompressed_latin1_text
            .iter()
            .map(|c| (c.keyword.clone(), c.text.clone()))
            .collect();
        assert!(chunks.contains(&("Thumb::URI".into(), "file:///pics/a%20b.png".into())));
        assert!(chunks.contains(&("Thumb::MTime".into(), "5".into())));
        let mode = std::fs::metadata(&written).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        assert!(!written.with_extension("png.tmp").exists());
    }

    #[test]
    fn a_thumbnail_other_programs_wrote_is_read() {
        // Written the way another program would: RGB, no Thumb::Size.
        let path = file("/pics/x.png");
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .add_text_chunk("Thumb::URI".into(), "file:///pics/x.png".into())
            .unwrap();
        encoder
            .add_text_chunk("Thumb::MTime".into(), "9".into())
            .unwrap();
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[1, 2, 3, 4, 5, 6]).unwrap();
        writer.finish().unwrap();
        let scratch = tempfile::tempdir().unwrap();
        let cache = cache(&scratch);
        let at = cache.path(&path, ThumbSize::Normal);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(&at, &bytes).unwrap();
        let found = cache.lookup(&path, &stamp(9), ThumbSize::Normal).unwrap();
        assert_eq!(
            found.map(|p| p.rgba().to_vec()),
            Some(vec![1, 2, 3, 255, 4, 5, 6, 255])
        );
    }

    #[test]
    fn a_damaged_thumbnail_is_no_thumbnail() {
        let scratch = tempfile::tempdir().unwrap();
        let cache = cache(&scratch);
        let path = file("/pics/cat.png");
        let at = cache.path(&path, ThumbSize::Normal);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(&at, b"not a png").unwrap();
        assert_eq!(
            cache.lookup(&path, &stamp(1), ThumbSize::Normal).unwrap(),
            None
        );
    }
}
