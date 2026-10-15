//! The hit a find is on and the address a link may open.

use crate::{FindHits, HitIndex, PdfStage, Stage};
use dioxus::prelude::*;

/// The hit the machine is on, when a find has one.
pub(super) fn current_hit(stage: &Stage) -> Option<HitIndex> {
    match stage {
        Stage::Pdf(PdfStage::Finding { hits, .. }) => match hits {
            FindHits::Found(_) => hits.current(),
            FindHits::Idle | FindHits::Pending | FindHits::NoMatch => None,
        },
        Stage::Pdf(PdfStage::Reading { .. } | PdfStage::Jumping { .. })
        | Stage::NoStage
        | Stage::Raster(_)
        | Stage::Media(_)
        | Stage::Text(_)
        | Stage::Table(_)
        | Stage::Tree(_) => None,
    }
}
/// The web address a link may open: only the schemes a person expects a PDF to open.
pub(super) fn web_address(uri: &str) -> Option<String> {
    let (scheme, _) = uri.split_once(':')?;
    const SCHEMES: [&str; 3] = ["http", "https", "mailto"];
    SCHEMES
        .iter()
        .any(|allowed| scheme.eq_ignore_ascii_case(allowed))
        .then(|| uri.to_owned())
}
