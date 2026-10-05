//! Image files as the pages of a PDF.

use crate::error::ExportError;
use anyview_core::FilePath;
use anyview_image::ImageFile;
use anyview_pdf::PagePicture;

/// The page `file` is placed on: an SVG stays vector, a JPEG that is already upright is stored as
/// it is, and anything else (another format, a turned photograph) is drawn and stored as pixels.
pub(crate) fn page_picture(file: &FilePath) -> Result<PagePicture, ExportError> {
    let image = ImageFile::read(file)?;
    if let Some(size) = image.vector_size()? {
        let svg = String::from_utf8_lossy(&image.bytes).into_owned();
        return Ok(PagePicture::Vector { svg, size });
    }
    if let Some(size) = image.upright_jpeg_size()? {
        return Ok(PagePicture::Jpeg {
            bytes: image.bytes,
            size,
        });
    }
    let picture = image.picture()?;
    Ok(PagePicture::Pixels {
        size: picture.size(),
        rgba: picture.into_bytes(),
    })
}
