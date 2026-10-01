//! The page a print or PDF export lays text out on.

use ds_core::word::Word;

/// A paper size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Word)]
pub enum PaperSize {
    /// ISO A4.
    #[default]
    A4,
    /// ISO A3.
    A3,
    /// US Letter.
    Letter,
    /// US Legal.
    Legal,
}

/// Which way up the page is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Word)]
pub enum Orientation {
    /// Taller than wide.
    #[default]
    Portrait,
    /// Wider than tall.
    Landscape,
}

/// The page text is printed to: a paper and which way up it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PrintLayout {
    /// The paper.
    pub paper: PaperSize,
    /// Which way up.
    pub orientation: Orientation,
}
