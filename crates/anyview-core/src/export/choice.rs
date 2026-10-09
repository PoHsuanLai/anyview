//! `ExportChoice`: what one format's export sheet is generic over.

use super::extension::ExportExtension;
use ds_core::word::Word;

/// A format's closed set of exports, with the options each carries. Each format has its own
/// implementation, so a combination that makes no sense (a page range on a photo) cannot be
/// written. The export sheet is generic over it: a format list over [`ExportChoice::kinds`] plus the
/// chosen variant's option fields.
pub trait ExportChoice: Clone + PartialEq + 'static {
    /// The variants without their options: what the dialog's format list shows.
    type Kind: Word;

    /// What the dialog's format list shows, in order. Every kind by default.
    fn kinds() -> &'static [Self::Kind] {
        <Self::Kind as Word>::ALL
    }

    /// Which kind this choice is: the list's selection.
    fn kind(&self) -> Self::Kind;

    /// `kind` with its options pre-filled. A launcher's "Convert to…" uses this as it stands.
    fn default_for(kind: Self::Kind) -> Self;

    /// The extension of the file this choice writes.
    fn extension(&self) -> ExportExtension;
}
