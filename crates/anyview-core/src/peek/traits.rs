//! The `Peek` trait.

use super::PeekBudget;
use crate::facts::Facts;
use crate::kind::FormatKind;
use crate::sniff::Sniffed;
use crate::source::Source;

/// The light tier of one format: cheap, no GPU, no libmpv. It runs in the launcher pane and as the
/// viewer's first frame. Each format implements it once, and a generic consumer runs over the
/// implementations (CONVENTIONS section 5).
pub trait Peek: 'static {
    /// The kind of file this implementation peeks at.
    const KIND: FormatKind;
    /// What a peek produces: a downscaled image, the first lines, an archive listing.
    type Peeked: Clone + PartialEq + Send + 'static;
    /// Why a peek failed.
    type Error: std::error::Error + Send;

    /// Looks at `src`, whose type `sniffed` established. Blocking: run it on a worker. Stays inside
    /// `budget` (bytes read, pixels decoded, time).
    fn peek(
        src: &Source,
        sniffed: &Sniffed,
        budget: &PeekBudget,
    ) -> Result<Self::Peeked, Self::Error>;

    /// The rows a pane lists about what was peeked at: dimensions, pages, duration…
    fn facts(peeked: &Self::Peeked) -> Facts;
}
