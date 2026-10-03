//! How large content is drawn.

use super::ratio::Permille;

/// A zoom level. `Fit` and `Fill` follow the window; `Actual` is one content pixel per device
/// pixel; `Scale` is a fixed proportion with 1000 permille as `Actual`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum Zoom {
    /// The whole content visible inside the window.
    Fit,
    /// The window filled, cropping the content.
    Fill,
    /// One content pixel per device pixel.
    Actual,
    /// A fixed proportion of actual size.
    Scale(Permille),
}

impl Zoom {
    /// The smallest scale [`Zoom::scaled`] gives: 1% of actual size.
    pub const MIN_SCALE: Permille = Permille(10);
    /// The largest scale [`Zoom::scaled`] gives: 6400% of actual size.
    pub const MAX_SCALE: Permille = Permille(64_000);

    /// A fixed scale, with `scale` clamped into [`Zoom::MIN_SCALE`] to [`Zoom::MAX_SCALE`].
    pub fn scaled(scale: Permille) -> Zoom {
        Zoom::Scale(Permille(
            scale.0.clamp(Self::MIN_SCALE.0, Self::MAX_SCALE.0),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaled_clamps_to_the_limits() {
        const CASES: &[(&str, u32, u32)] = &[
            ("below", 0, 10),
            ("lowest", 10, 10),
            ("actual", 1000, 1000),
            ("highest", 64_000, 64_000),
            ("above", 64_001, 64_000),
        ];
        for (name, given, want) in CASES {
            assert_eq!(
                Zoom::scaled(Permille(*given)),
                Zoom::Scale(Permille(*want)),
                "{name}"
            );
        }
    }

    #[test]
    fn zoom_round_trips_adjacently_tagged() {
        const CASES: &[(&str, Zoom, &str)] = &[
            ("fit", Zoom::Fit, r#"{"kind":"fit"}"#),
            ("fill", Zoom::Fill, r#"{"kind":"fill"}"#),
            ("actual", Zoom::Actual, r#"{"kind":"actual"}"#),
            (
                "scale",
                Zoom::Scale(Permille(250)),
                r#"{"kind":"scale","v":250}"#,
            ),
        ];
        for (name, zoom, json) in CASES {
            assert_eq!(&serde_json::to_string(zoom).unwrap(), json, "{name}");
            assert_eq!(&serde_json::from_str::<Zoom>(json).unwrap(), zoom, "{name}");
        }
    }
}
