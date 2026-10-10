//! A comic bound as a PDF: each picture is a page of its own size. The pictures are put on pages a
//! few at a time, so a comic of large pages is never all decoded at once.

use crate::io::OpenError;
use anyview_book::{BookError, Comic};
use anyview_core::{FactLabel, FactValue, Facts, FileName, FilePath, SectionIndex};
use anyview_fs::OnDisk;
use anyview_image::ImageFile;
use anyview_pdf::{PagePicture, bind as bind_parts, pdf_of_pictures};

/// How many pictures are put on pages before the pages are written out.
const BATCH: usize = 12;

/// The page `image` is placed on: a JPEG that is already upright is stored as it is, anything else
/// is drawn and stored as pixels.
fn picture(image: ImageFile) -> Option<PagePicture> {
    if let Some(size) = image.upright_jpeg_size().ok().flatten() {
        return Some(PagePicture::Jpeg {
            bytes: image.bytes,
            size,
        });
    }
    let picture = image.picture().ok()?;
    Some(PagePicture::Pixels {
        size: picture.size(),
        rgba: picture.into_bytes(),
    })
}

/// The comic at `path` as the bytes of one PDF, and the rows of its Info tab (`base` and the
/// page count).
pub(super) fn bind(path: &FilePath, base: Facts) -> Result<(Vec<u8>, Facts), OpenError> {
    let comic = Comic::open(path.on_disk())?;
    let facts = base.with(
        FactLabel::Pages,
        FactValue::text(comic.sections().get().to_string()),
    );
    // A picture that cannot be read is left out; the rest of the comic still reads.
    let mut parts: Vec<Vec<u8>> = Vec::new();
    let mut batch: Vec<PagePicture> = Vec::new();
    for at in 0..comic.sections().get() {
        let Ok((page, bytes)) = comic.page(SectionIndex(at)) else {
            continue;
        };
        let name = page.entry.rsplit('/').next().unwrap_or(&page.entry);
        let Some(page) = FileName::new(name)
            .ok()
            .and_then(|name| ImageFile::from_bytes(bytes, &name).ok())
            .and_then(picture)
        else {
            continue;
        };
        batch.push(page);
        if batch.len() == BATCH {
            parts.push(pdf_of_pictures(&std::mem::take(&mut batch)).map_err(|_| BookError::Empty)?);
        }
    }
    if !batch.is_empty() {
        parts.push(pdf_of_pictures(&batch).map_err(|_| BookError::Empty)?);
    }
    if parts.is_empty() {
        return Err(BookError::Empty.into());
    }
    let bytes = bind_parts(&parts, &[]).map_err(|_| BookError::Empty)?;
    Ok((bytes, facts))
}
