//! A text document laid out on paper as a vector PDF.

use crate::error::ExportError;
use crate::session::Session;
use anyview_core::{HtmlDoc, Orientation, PaperSize, PrintLayout};
use ds_blitz::{PageSize, PageSpec, Pt};

/// `doc` printed on the paper `layout` names, with quire's margins.
pub(crate) fn print_html(
    session: &mut Session,
    doc: &HtmlDoc,
    layout: PrintLayout,
) -> Result<Vec<u8>, ExportError> {
    let html = anyview_text::printable_html(doc, session.highlighter())?;
    Ok(ds_blitz::pdf(&html, page_spec(layout))?)
}

/// The page of a layout: the paper, turned on its side for landscape.
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

#[cfg(test)]
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
