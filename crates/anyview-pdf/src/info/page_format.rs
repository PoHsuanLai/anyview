//! A page size as a person names it: A4, Letter, or `210 × 297 mm`.

use crate::geometry::PageSize;

/// Which way a page is turned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Orientation {
    /// Taller than wide, or square.
    Portrait,
    /// Wider than tall.
    Landscape,
}

/// How big the pages of a document are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PageFormat {
    /// A standard size.
    Named {
        /// `A4`, `Letter`.
        name: &'static str,
        /// Which way the pages are turned.
        orientation: Orientation,
    },
    /// Any other size, in whole millimetres.
    Custom {
        /// The width.
        width_mm: u32,
        /// The height.
        height_mm: u32,
    },
    /// The pages are not all one size.
    Varies,
}

/// The standard sizes, portrait, in whole millimetres.
const STANDARD: &[(&str, u32, u32)] = &[
    ("A3", 297, 420),
    ("A4", 210, 297),
    ("A5", 148, 210),
    ("Letter", 216, 279),
    ("Legal", 216, 356),
    ("Tabloid", 279, 432),
];

/// How far a page may be from a standard size, in millimetres, and still be it.
const TOLERANCE_MM: u32 = 1;

impl PageFormat {
    /// The words for the format: `A4`, `Letter, landscape`, `210 × 297 mm`, `Varies`.
    pub fn text(self) -> String {
        match self {
            PageFormat::Named {
                name,
                orientation: Orientation::Portrait,
            } => name.to_owned(),
            PageFormat::Named {
                name,
                orientation: Orientation::Landscape,
            } => format!("{name}, landscape"),
            PageFormat::Custom {
                width_mm,
                height_mm,
            } => format!("{width_mm} × {height_mm} mm"),
            PageFormat::Varies => "Varies".to_owned(),
        }
    }

    fn of_page(size: PageSize) -> PageFormat {
        // Thousandths of a point to whole millimetres: 25.4 mm to 72 points.
        let mm = |length: u32| (u64::from(length) * 254 + 360_000) / 720_000;
        let (width, height) = (
            u32::try_from(mm(size.width.0)).unwrap_or(u32::MAX),
            u32::try_from(mm(size.height.0)).unwrap_or(u32::MAX),
        );
        let (short, long, orientation) = if width > height {
            (height, width, Orientation::Landscape)
        } else {
            (width, height, Orientation::Portrait)
        };
        let near = |a: u32, b: u32| a.abs_diff(b) <= TOLERANCE_MM;
        STANDARD
            .iter()
            .find(|(_, w, h)| near(short, *w) && near(long, *h))
            .map_or(
                PageFormat::Custom {
                    width_mm: width,
                    height_mm: height,
                },
                |(name, _, _)| PageFormat::Named { name, orientation },
            )
    }
}

/// The format of the document whose pages are `sizes`: the first page's when they all agree.
pub(super) fn of_pages(sizes: &[PageSize]) -> PageFormat {
    let mut formats = sizes.iter().copied().map(PageFormat::of_page);
    let Some(first) = formats.next() else {
        return PageFormat::Varies;
    };
    if formats.all(|format| format == first) {
        first
    } else {
        PageFormat::Varies
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::MilliPoints;

    fn page(width_pt: f64, height_pt: f64) -> PageSize {
        PageSize {
            width: MilliPoints::from_points(width_pt),
            height: MilliPoints::from_points(height_pt),
        }
    }

    #[test]
    fn pages_are_named_when_they_are_a_standard_size() {
        // name, pages, words
        let cases: &[(&str, Vec<PageSize>, &str)] = &[
            ("a4", vec![page(595.28, 841.89)], "A4"),
            (
                "a4 as word processors round it",
                vec![page(595.0, 842.0)],
                "A4",
            ),
            ("letter", vec![page(612.0, 792.0)], "Letter"),
            ("legal", vec![page(612.0, 1008.0)], "Legal"),
            ("a4 landscape", vec![page(841.89, 595.28)], "A4, landscape"),
            ("a5", vec![page(419.53, 595.28)], "A5"),
            ("custom", vec![page(400.0, 300.0)], "141 × 106 mm"),
            ("many a4", vec![page(595.28, 841.89); 5], "A4"),
            (
                "mixed",
                vec![page(595.28, 841.89), page(612.0, 792.0)],
                "Varies",
            ),
            (
                "portrait then landscape",
                vec![page(595.28, 841.89), page(841.89, 595.28)],
                "Varies",
            ),
        ];
        for (name, pages, want) in cases {
            assert_eq!(of_pages(pages).text(), *want, "{name}");
        }
    }
}
