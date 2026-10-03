//! The scales tiles are drawn at. A view zooms continuously; tiles are drawn at a few steps of it
//! (four to the doubling) so a pinch does not redraw the page at every frame, and a tile is never
//! drawn smaller than it is shown.

use anyview_core::Permille;

/// A step of the ladder of draw scales: bucket `k` draws at 2 to the power `k / 4` times actual
/// size (a point as a pixel), so bucket 0 is 1000 permille and every fourth step doubles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ZoomBucket(i8);

/// The scale of bucket 0, 1, 2 and 3 in the first octave, in thousandths.
const OCTAVE: [u64; 4] = [1000, 1189, 1414, 1682];

impl ZoomBucket {
    /// The lowest bucket: about 11 permille, just above the lowest zoom the viewer allows.
    pub const MIN: ZoomBucket = ZoomBucket(-26);
    /// The highest bucket: 64 times actual size, the highest zoom the viewer allows.
    pub const MAX: ZoomBucket = ZoomBucket(24);

    /// The scale this bucket draws at.
    pub fn scale(self) -> Permille {
        let octave = self.0.div_euclid(4);
        let step = usize::from(self.0.rem_euclid(4).unsigned_abs());
        let base = OCTAVE.get(step).copied().unwrap_or(1000);
        let scaled = if octave >= 0 {
            base << octave.unsigned_abs()
        } else {
            let shift = octave.unsigned_abs();
            (base + (1u64 << (shift - 1))) >> shift
        };
        Permille(u32::try_from(scaled).unwrap_or(u32::MAX))
    }

    /// The lowest bucket that draws at `shown` or larger, so what is shown is scaled down from
    /// what was drawn. A scale past the ladder is its end.
    pub fn containing(shown: Permille) -> ZoomBucket {
        (Self::MIN.0..=Self::MAX.0)
            .map(ZoomBucket)
            .find(|bucket| bucket.scale() >= shown)
            .unwrap_or(Self::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buckets_double_every_fourth_step() {
        const CASES: &[(i8, u32)] = &[
            (0, 1000),
            (1, 1189),
            (2, 1414),
            (3, 1682),
            (4, 2000),
            (24, 64_000),
            (-4, 500),
            (-8, 250),
            (-26, 11),
        ];
        for (step, want) in CASES {
            assert_eq!(ZoomBucket(*step).scale(), Permille(*want), "bucket {step}");
        }
    }

    #[test]
    fn a_scale_takes_the_lowest_bucket_that_covers_it() {
        const CASES: &[(&str, u32, i8)] = &[
            ("exact", 1000, 0),
            ("just over", 1001, 1),
            ("between", 1300, 2),
            ("half", 500, -4),
            ("below the ladder", 1, -26),
            ("at the lowest zoom", 10, -26),
            ("past the ladder", 70_000, 24),
        ];
        for (name, shown, want) in CASES {
            let bucket = ZoomBucket::containing(Permille(*shown));
            assert_eq!(bucket, ZoomBucket(*want), "{name}");
            if bucket != ZoomBucket::MAX {
                assert!(
                    bucket.scale() >= Permille(*shown),
                    "{name}: covers the scale"
                );
            }
        }
    }
}
