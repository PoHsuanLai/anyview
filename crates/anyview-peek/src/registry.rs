//! The light tier's registry: the one exhaustive match over `FormatKind` for the peeks.
//!
//! A new kind is a compile error here until it names its `Peek`. `anyview-ui` keeps the matching
//! registry for the full tier; `anyview-core` cannot hold either, as it cannot name an
//! implementation.

use crate::body::Light;
use crate::described::{ArchivePeek, BookPeek, FontPeek, OfficePeek, OtherPeek};
use crate::folder::FolderPeek;
use crate::media::{AudioPeek, VideoPeek};
use crate::pdf::PdfPeek;
use anyview_core::FormatKind;
use anyview_image::{RasterPeek, VectorPeek};
use anyview_text::{CodePeek, MarkdownPeek, PlainPeek, TablePeek, TreePeek};

/// Something to do with the peek of a kind, written once and run for any of them. The visitor
/// names the type, so a generic body can use `P::peek`, `P::facts` and `P::KIND`.
pub trait KindVisitor {
    /// What the visit returns.
    type Out;

    /// Runs the visitor for the kind whose peek is `P`.
    fn visit<P: Light>(self) -> Self::Out;
}

/// Runs `visitor` for the peek of `kind`: the one exhaustive match over `FormatKind` in the light
/// tier.
pub fn visit<V: KindVisitor>(kind: FormatKind, visitor: V) -> V::Out {
    match kind {
        FormatKind::Pdf => visitor.visit::<PdfPeek>(),
        FormatKind::Raster => visitor.visit::<RasterPeek>(),
        FormatKind::Vector => visitor.visit::<VectorPeek>(),
        FormatKind::Video => visitor.visit::<VideoPeek>(),
        FormatKind::Audio => visitor.visit::<AudioPeek>(),
        FormatKind::Markdown => visitor.visit::<MarkdownPeek>(),
        FormatKind::Code => visitor.visit::<CodePeek>(),
        FormatKind::PlainText => visitor.visit::<PlainPeek>(),
        FormatKind::Table => visitor.visit::<TablePeek>(),
        FormatKind::Tree => visitor.visit::<TreePeek>(),
        FormatKind::Font => visitor.visit::<FontPeek>(),
        FormatKind::Archive => visitor.visit::<ArchivePeek>(),
        FormatKind::Book => visitor.visit::<BookPeek>(),
        FormatKind::Office => visitor.visit::<OfficePeek>(),
        FormatKind::Folder => visitor.visit::<FolderPeek>(),
        FormatKind::Other => visitor.visit::<OtherPeek>(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ds::prelude::Word;

    /// Reports the kind a peek is registered for, so the table below can compare it with the kind
    /// the registry was asked about.
    struct KindOf;

    impl KindVisitor for KindOf {
        type Out = FormatKind;

        fn visit<P: Light>(self) -> FormatKind {
            P::KIND
        }
    }

    #[test]
    fn every_kind_maps_to_the_peek_of_that_kind() {
        for kind in FormatKind::ALL {
            assert_eq!(visit(*kind, KindOf), *kind, "{}", kind.slug());
        }
    }
}
