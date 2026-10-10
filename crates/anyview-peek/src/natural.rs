//! The size a file's picture shows at, for sizing a window to it before it opens. Only the start
//! of the file is read (a picture's header, a movie's header) and nothing is decoded, so it takes a
//! few milliseconds whatever the file claims; see [`natural_size`].

use crate::media::video_size;
use anyview_core::{FileHead, FormatKind, Input, PixelSize, SniffStep, sniff};
use std::panic::{AssertUnwindSafe, catch_unwind};

/// The size of the picture of the file `src` (a path, or any bytes a host injects), in pixels and
/// upright: a raster picture or SVG (the size it declares) or a video (its resolution). `None` for
/// everything else, a path that is not a regular file, and a file whose header does not say or
/// cannot be read; the caller then uses its default. The size is the file's own claim and may be
/// absurd. Blocking, and bounded: at most a quarter of a mebibyte of a picture and eight of a
/// movie's header are read.
#[must_use]
pub fn natural_size(src: impl Into<Input>) -> Option<PixelSize> {
    let src = src.into();
    let head = src.bytes().read_range(0..FileHead::MAX.0).ok()?;
    let SniffStep::Done(sniffed) = sniff(&FileHead::new(&head), src.name()) else {
        return None;
    };
    // A back end fed a hostile header may panic: that is no size, not a dead worker.
    catch_unwind(AssertUnwindSafe(|| match sniffed.kind() {
        FormatKind::Raster | FormatKind::Vector => anyview_image::natural_size(&src, &sniffed),
        FormatKind::Video => video_size(&src, &sniffed),
        FormatKind::Pdf
        | FormatKind::Audio
        | FormatKind::Markdown
        | FormatKind::Code
        | FormatKind::PlainText
        | FormatKind::Table
        | FormatKind::Tree
        | FormatKind::Font
        | FormatKind::Archive
        | FormatKind::Book
        | FormatKind::Office
        | FormatKind::Folder
        | FormatKind::Other => None,
    }))
    .ok()
    .flatten()
}

/// Whether `src` (a path, or any bytes a host injects) is an audio file, by its head. Only the
/// first bytes are read, nothing is decoded and no tag is parsed, so it is cheap for any caller.
/// `false` for anything else, a path that is not a regular file, and bytes that cannot be read.
#[must_use]
pub fn is_audio(src: impl Into<Input>) -> bool {
    let src = src.into();
    let Ok(head) = src.bytes().read_range(0..FileHead::MAX.0) else {
        return false;
    };
    matches!(
        sniff(&FileHead::new(&head), src.name()),
        SniffStep::Done(sniffed) if sniffed.kind() == FormatKind::Audio
    )
}
