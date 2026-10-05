//! Pictures as the pages of a PDF: a photograph, pixels or an SVG, each on a page of its own.

use crate::edit::save;
use crate::error::PdfError;
use anyview_core::PixelSize;
use pdfrum::{Limits, Rect};
use pdfrum_edit::{EditDoc, PixelFormat, Size, SvgFit, blank_document};

/// A CSS pixel in points: the size a picture with no resolution of its own is given.
const POINTS_PER_PIXEL: f64 = 0.75;

/// The longest side of a page, in points (an A4 sheet's long side). A larger picture is scaled
/// down to it, so a twelve-megapixel photograph is a page a printer can take, not a poster.
const LONG_SIDE: f64 = 842.0;

/// What goes on one page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PagePicture {
    /// A JPEG file, stored in the PDF as it is: nothing is decoded or compressed again. Only a
    /// file that is already upright qualifies, since a PDF reads no EXIF orientation.
    Jpeg {
        /// The file.
        bytes: Vec<u8>,
        /// The size it declares.
        size: PixelSize,
    },
    /// Upright pixels, straight RGBA8; transparency stays transparent.
    Pixels {
        /// The size.
        size: PixelSize,
        /// Four bytes a pixel.
        rgba: Vec<u8>,
    },
    /// An SVG document, drawn as vector paths at the size it declares.
    Vector {
        /// The document.
        svg: String,
        /// The size it declares.
        size: PixelSize,
    },
}

impl PagePicture {
    /// The page this picture is placed on: its size in points, shrunk to fit [`LONG_SIDE`].
    fn page(&self) -> Size {
        let (PagePicture::Jpeg { size, .. }
        | PagePicture::Pixels { size, .. }
        | PagePicture::Vector { size, .. }) = self;
        let natural = (
            f64::from(size.width.0) * POINTS_PER_PIXEL,
            f64::from(size.height.0) * POINTS_PER_PIXEL,
        );
        let shrink = (LONG_SIDE / natural.0.max(natural.1)).min(1.0);
        Size::new(natural.0 * shrink, natural.1 * shrink)
    }
}

/// A PDF with one page for each of `pictures`, each page the size of its picture.
pub fn pdf_of_pictures(pictures: &[PagePicture]) -> Result<Vec<u8>, PdfError> {
    let sizes: Vec<Size> = pictures.iter().map(PagePicture::page).collect();
    if sizes.is_empty() {
        return Err(PdfError::NoPages);
    }
    let blank = blank_document(&sizes)?;
    let mut edit = EditDoc::new(&blank);
    for (index, (picture, page)) in pictures.iter().zip(&sizes).enumerate() {
        let whole = Rect::new(0.0, 0.0, page.width, page.height);
        place(&mut edit, index, picture, whole)?;
    }
    save(&edit)
}

/// `picture` drawn over `whole`, the page at `index`.
fn place(
    edit: &mut EditDoc<'_>,
    index: usize,
    picture: &PagePicture,
    whole: Rect,
) -> Result<(), PdfError> {
    let limits = Limits::default();
    let page = u32::try_from(index).unwrap_or(u32::MAX);
    match picture {
        PagePicture::Jpeg { bytes, .. } => {
            let image = edit.embed_jpeg(bytes)?;
            edit.draw_page(page, &limits, |canvas| canvas.image(&image, whole))?;
        }
        PagePicture::Pixels { size, rgba } => {
            let image = edit.embed_image(rgba, size.width.0, size.height.0, PixelFormat::Rgba8)?;
            edit.draw_page(page, &limits, |canvas| canvas.image(&image, whole))?;
        }
        PagePicture::Vector { svg, .. } => {
            let mut drawn = Ok(());
            edit.draw_page(page, &limits, |canvas| {
                drawn = canvas.draw_svg(svg, whole, SvgFit::Contain).map(|_| ());
            })?;
            drawn?;
        }
    }
    Ok(())
}
