//! The export of a format that has none.

use super::choice::ExportChoice;
use super::extension::ExportExtension;
use ds_core::word::Word;

/// A format with no export: uninhabited, so there is no value an export sheet could be opened
/// on, and the type system says so where a sheet would be generic over it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoExport {}

/// The kinds of [`NoExport`]: none, so its pop-up lists nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoExportKind {}

// `#[derive(Word)]` refuses an enum with no variant, so the one empty vocabulary is written by
// hand: no variant to list, and `slug` and `label` can never be called.
impl Word for NoExportKind {
    const ALL: &'static [Self] = &[];

    fn slug(self) -> &'static str {
        match self {}
    }

    fn label(self) -> &'static str {
        match self {}
    }
}

impl ExportChoice for NoExport {
    type Kind = NoExportKind;

    fn kind(&self) -> NoExportKind {
        match *self {}
    }

    fn default_for(kind: NoExportKind) -> Self {
        match kind {}
    }

    fn extension(&self) -> ExportExtension {
        match *self {}
    }
}
