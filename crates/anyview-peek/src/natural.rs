//! The size a file's picture shows at, for sizing a window to it before it opens. Only the start
//! of the file is read (a picture's header, a movie's header) and nothing is decoded, so it takes a
//! few milliseconds whatever the file claims; see [`natural_size`].

use crate::media::video_size;
use anyview_core::{
    ByteLen, FileHead, FilePath, FileStamp, FormatKind, ModTime, PixelSize, SniffStep, Source,
    open_regular, sniff,
};
use std::io::Read;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// The size of the picture of the file at `path`, in pixels and upright: a raster picture or SVG
/// (the size it declares) or a video (its resolution). `None` for everything else, a path that is
/// not a regular file, and a file whose header does not say or cannot be read; the caller then
/// uses its default. The size is the file's own claim and may be absurd. Blocking, and bounded:
/// at most a quarter of a mebibyte of a picture and eight of a movie's header are read.
pub fn natural_size(path: &FilePath) -> Option<PixelSize> {
    let (mut file, meta) = open_regular(path.as_path()).ok()?;
    let mut head = Vec::new();
    (&mut file)
        .take(FileHead::MAX.0)
        .read_to_end(&mut head)
        .ok()?;
    let SniffStep::Done(sniffed) = sniff(&FileHead::new(&head), &path.file_name()?) else {
        return None;
    };
    // A back end fed a hostile header may panic: that is no size, not a dead worker.
    catch_unwind(AssertUnwindSafe(|| match sniffed.kind() {
        FormatKind::Raster | FormatKind::Vector => anyview_image::natural_size(path, &sniffed),
        FormatKind::Video => {
            let stamp = FileStamp {
                len: ByteLen(meta.len()),
                modified: meta
                    .modified()
                    .map_or(ModTime(0), ModTime::from_system_time),
            };
            video_size(&Source::new(path.clone(), stamp), &sniffed)
        }
        _ => None,
    }))
    .ok()
    .flatten()
}
