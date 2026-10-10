//! The `Peek` trait.

use super::PeekBudget;
use crate::facts::Facts;
use crate::kind::FormatKind;
use crate::sniff::Sniffed;
use crate::source::Input;
use crate::units::PixelSize;

/// The light tier of one format: cheap, no GPU, no player. It runs in the launcher pane and as the
/// viewer's first frame. Each format implements it once, and a generic consumer runs over the
/// implementations (CONVENTIONS section 5).
pub trait Peek: 'static {
    /// The kind of file this implementation peeks at.
    const KIND: FormatKind;
    /// What a peek produces: a downscaled image, the first lines, an archive listing.
    type Peeked: Clone + PartialEq + Send + 'static;
    /// Why a peek failed.
    type Error: std::error::Error + Send;

    /// Looks at `src` (a path, or any bytes a host injects), whose type `sniffed` established.
    /// Blocking: run it on a worker. Stays inside `budget` (bytes read, pixels decoded, time).
    fn peek(
        src: &Input,
        sniffed: &Sniffed,
        budget: &PeekBudget,
    ) -> Result<Self::Peeked, Self::Error>;

    /// [`Peek::peek`], told the box, in pixels, that what it draws is fitted into: a PDF's first
    /// page is rasterised for it. A format that draws nothing, or draws by the budget's pixels
    /// alone, ignores it, as this default does.
    fn peek_fitted(
        src: &Input,
        sniffed: &Sniffed,
        budget: &PeekBudget,
        fit: PixelSize,
    ) -> Result<Self::Peeked, Self::Error> {
        let _ = fit;
        Self::peek(src, sniffed, budget)
    }

    /// The rows a pane lists about what was peeked at: dimensions, pages, duration…
    fn facts(peeked: &Self::Peeked) -> Facts;
}
