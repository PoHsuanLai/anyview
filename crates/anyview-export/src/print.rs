//! A text document laid out on paper as a vector PDF.

use crate::error::ExportError;
use crate::session::Session;
use anyview_core::{HtmlDoc, PrintLayout};
#[cfg(feature = "print")]
use anyview_core::{Orientation, PaperSize};
#[cfg(feature = "print")]
use ds_blitz::{PageSize, PageSpec, Pt};

/// `doc` printed on the paper `layout` names, with quire's margins.
#[cfg(feature = "print")]
pub(crate) fn print_html(
    session: &mut Session,
    doc: &HtmlDoc,
    layout: PrintLayout,
) -> Result<Vec<u8>, ExportError> {
    let html = anyview_text::printable_html(doc, session.highlighter())?;
    Ok(ds_blitz::pdf(&html, page_spec(layout))?)
}

/// A text document cannot be laid out on paper without the renderer.
#[cfg(not(feature = "print"))]
pub(crate) fn print_html(
    _session: &mut Session,
    _doc: &HtmlDoc,
    _layout: PrintLayout,
) -> Result<Vec<u8>, ExportError> {
    Err(ExportError::NoRenderer)
}

/// The page of a layout: the paper, turned on its side for landscape.
#[cfg(feature = "print")]
fn page_spec(layout: PrintLayout) -> PageSpec {
    let (across, down) = match layout.paper {
        PaperSize::A4 => (Pt::from_mm(210.0), Pt::from_mm(297.0)),
        PaperSize::A3 => (Pt::from_mm(297.0), Pt::from_mm(420.0)),
        PaperSize::Letter => (Pt::from_inches(8.5), Pt::from_inches(11.0)),
        PaperSize::Legal => (Pt::from_inches(8.5), Pt::from_inches(14.0)),
    };
    let (width, height) = match layout.orientation {
        Orientation::Portrait => (across, down),
        Orientation::Landscape => (down, across),
    };
    PageSpec {
        size: PageSize::Custom { width, height },
        ..PageSpec::default()
    }
}

#[cfg(all(test, feature = "print"))]
mod tests {
    use super::*;

    #[test]
    fn landscape_turns_the_paper_on_its_side() {
        let page = |paper, orientation| {
            page_spec(PrintLayout { paper, orientation })
                .size
                .dimensions()
        };
        let (width, height) = page(PaperSize::Letter, Orientation::Portrait);
        assert!(width < height, "portrait is taller than wide");
        assert_eq!(
            page(PaperSize::Letter, Orientation::Landscape),
            (height, width)
        );
    }
}
