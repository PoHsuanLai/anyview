//! Whether a kind has a full viewer stage or only a peek.

use ds_core::word::Word;

/// How far the viewer goes for a kind of file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum StageSupport {
    /// The viewer has a stage for it.
    Stage,
    /// Only the light tier: facts
    PeekOnly,
}
