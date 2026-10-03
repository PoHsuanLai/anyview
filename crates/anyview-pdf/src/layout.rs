//! Pages stacked top to bottom with a gap between them, at one scale: where each page sits, which
//! page is at a height, and the height a scroll position names. Pure arithmetic over page sizes.

use crate::geometry::{MilliPoints, PageSize};
use anyview_core::{PageIndex, Permille, PixelLen, PixelSize};

/// Where one page sits in the stack, in pixels at the layout's scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PagePlace {
    /// Distance from the top of the stack to the page's top edge.
    pub top: u64,
    /// Distance from the left of the stack to the page's left edge (pages are centred).
    pub left: u32,
    /// The page's size.
    pub size: PixelSize,
}

/// The stack of pages at one scale (thousandths of a pixel per point: 1000 draws a point as one
/// pixel).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageLayout {
    scale: Permille,
    sizes: Vec<PageSize>,
    places: Vec<PagePlace>,
    width: u32,
    height: u64,
}

/// `length` points at `scale`, in whole pixels (truncated as the rasterizer truncates a page's box)
/// and at least one.
pub(crate) fn pixels(length: MilliPoints, scale: Permille) -> u32 {
    let whole = u64::from(length.0) * u64::from(scale.0) / 1_000_000;
    u32::try_from(whole).unwrap_or(u32::MAX).max(1)
}

impl PageLayout {
    /// The stack of `sizes` at `scale`, `gap` pixels between pages.
    pub fn new(sizes: &[PageSize], scale: Permille, gap: PixelLen) -> PageLayout {
        let drawn: Vec<PixelSize> = sizes
            .iter()
            .map(|size| PixelSize {
                width: PixelLen(pixels(size.width, scale)),
                height: PixelLen(pixels(size.height, scale)),
            })
            .collect();
        let width = drawn.iter().map(|size| size.width.0).max().unwrap_or(0);
        let mut top = 0u64;
        let mut places = Vec::with_capacity(drawn.len());
        for size in &drawn {
            places.push(PagePlace {
                top,
                left: (width - size.width.0) / 2,
                size: *size,
            });
            top += u64::from(size.height.0) + u64::from(gap.0);
        }
        PageLayout {
            scale,
            sizes: sizes.to_vec(),
            places,
            width,
            height: top.saturating_sub(u64::from(gap.0)),
        }
    }

    /// The scale the stack is laid out at.
    pub fn scale(&self) -> Permille {
        self.scale
    }

    /// The page sizes the stack was made from.
    pub fn sizes(&self) -> &[PageSize] {
        &self.sizes
    }

    /// The width of the widest page.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// The height of the whole stack.
    pub fn height(&self) -> u64 {
        self.height
    }

    /// Where `page` sits.
    pub fn place(&self, page: PageIndex) -> Option<&PagePlace> {
        usize::try_from(page.0)
            .ok()
            .and_then(|at| self.places.get(at))
    }

    /// The height a view starting `offset` of the way down `page` is at. A page the stack does not
    /// hold is the end of the stack.
    pub fn y_of(&self, page: PageIndex, offset: Permille) -> u64 {
        match self.place(page) {
            Some(place) => {
                let down = u64::from(place.size.height.0) * u64::from(offset.0.min(1000)) / 1000;
                place.top + down
            }
            None => self.height,
        }
    }

    /// The page at height `y`, and how far down it that is. A height in a gap is the start of the
    /// page below it; one past the end is the end of the last page.
    pub fn at(&self, y: u64) -> (PageIndex, Permille) {
        let below = self.places.partition_point(|place| place.top <= y);
        let Some(at) = below.checked_sub(1) else {
            return (PageIndex(0), Permille(0));
        };
        let Some(place) = self.places.get(at) else {
            return (PageIndex(0), Permille(0));
        };
        let page = PageIndex(u32::try_from(at).unwrap_or(u32::MAX));
        let down = y - place.top;
        let high = u64::from(place.size.height.0);
        if down >= high {
            // In the gap after this page: the start of the next one, or the end of the last.
            return match self.places.get(at + 1) {
                Some(_) => (PageIndex(page.0 + 1), Permille(0)),
                None => (page, Permille(1000)),
            };
        }
        (
            page,
            Permille(u32::try_from(down * 1000 / high).unwrap_or(1000)),
        )
    }

    /// Every page with any part in `y_from..y_to`, top to bottom.
    pub fn between(&self, y_from: u64, y_to: u64) -> impl Iterator<Item = (PageIndex, &PagePlace)> {
        let first = self
            .places
            .partition_point(|place| place.top + u64::from(place.size.height.0) <= y_from);
        self.places
            .iter()
            .enumerate()
            .skip(first)
            .take_while(move |(_, place)| place.top < y_to)
            .map(|(at, place)| (PageIndex(u32::try_from(at).unwrap_or(u32::MAX)), place))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size(width_pt: u32, height_pt: u32) -> PageSize {
        PageSize {
            width: MilliPoints(width_pt * 1000),
            height: MilliPoints(height_pt * 1000),
        }
    }

    fn stack() -> PageLayout {
        // At 500 permille: 100 x 200, 200 x 100 and 100 x 200 pixels, a 10 pixel gap.
        PageLayout::new(
            &[size(200, 400), size(400, 200), size(200, 400)],
            Permille(500),
            PixelLen(10),
        )
    }

    #[test]
    fn pages_stack_with_gaps_and_are_centred() {
        let layout = stack();
        assert_eq!(
            (layout.width(), layout.height()),
            (200, 200 + 10 + 100 + 10 + 200)
        );
        let tops: Vec<(u64, u32)> = layout.places.iter().map(|p| (p.top, p.left)).collect();
        assert_eq!(tops, [(0, 50), (210, 0), (320, 50)]);
    }

    #[test]
    fn a_height_names_a_page_and_how_far_down_it() {
        const CASES: &[(&str, u64, u32, u32)] = &[
            // name, y, page, offset
            ("top", 0, 0, 0),
            ("middle of the first", 100, 0, 500),
            ("in the first gap", 205, 1, 0),
            ("start of the second", 210, 1, 0),
            ("a quarter down the second", 235, 1, 250),
            ("end of the stack", 520, 2, 1000),
            ("far past the end", 99_999, 2, 1000),
        ];
        let layout = stack();
        for (name, y, page, offset) in CASES {
            assert_eq!(
                layout.at(*y),
                (PageIndex(*page), Permille(*offset)),
                "{name}"
            );
        }
    }

    #[test]
    fn a_page_and_offset_name_a_height() {
        let layout = stack();
        assert_eq!(layout.y_of(PageIndex(1), Permille(500)), 260);
        assert_eq!(layout.y_of(PageIndex(0), Permille(0)), 0);
        assert_eq!(layout.y_of(PageIndex(9), Permille(0)), layout.height());
    }

    #[test]
    fn only_pages_touching_a_span_are_listed() {
        const CASES: &[(&str, u64, u64, &[u32])] = &[
            ("inside one", 20, 40, &[0]),
            ("across a gap", 190, 230, &[0, 1]),
            ("only the gap", 200, 210, &[]),
            ("everything", 0, 10_000, &[0, 1, 2]),
            ("beyond", 600, 700, &[]),
        ];
        let layout = stack();
        for (name, from, to, want) in CASES {
            let got: Vec<u32> = layout.between(*from, *to).map(|(page, _)| page.0).collect();
            assert_eq!(&got, want, "{name}");
        }
    }
}
