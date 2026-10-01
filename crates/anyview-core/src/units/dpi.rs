//! Resolution for rendering a page to pixels.

use crate::error::CoreError;

/// Dots per inch, 1 to 2400.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(try_from = "u32", into = "u32")]
pub struct Dpi(u16);

impl Dpi {
    /// The resolution a PDF page is laid out at: one point per pixel.
    pub const PAGE: Dpi = Dpi(72);
    /// A sharp screen render.
    pub const SCREEN: Dpi = Dpi(150);
    /// A print render.
    pub const PRINT: Dpi = Dpi(300);

    /// `value` dpi, or why it is outside 1 to 2400.
    pub fn new(value: u32) -> Result<Self, CoreError> {
        match u16::try_from(value) {
            Ok(dpi) if (1..=2400).contains(&dpi) => Ok(Dpi(dpi)),
            Ok(_) | Err(_) => Err(CoreError::DpiOutOfRange { value }),
        }
    }

    /// The resolution in dots per inch.
    pub fn get(self) -> u32 {
        u32::from(self.0)
    }
}

impl TryFrom<u32> for Dpi {
    type Error = CoreError;

    fn try_from(value: u32) -> Result<Self, CoreError> {
        Dpi::new(value)
    }
}

impl From<Dpi> for u32 {
    fn from(dpi: Dpi) -> u32 {
        dpi.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpi_accepts_one_to_twenty_four_hundred() {
        const CASES: &[(&str, u32, Option<u32>)] = &[
            ("zero", 0, None),
            ("lowest", 1, Some(1)),
            ("page", 72, Some(72)),
            ("highest", 2400, Some(2400)),
            ("above", 2401, None),
            ("beyond u16", 70_000, None),
        ];
        for (name, given, want) in CASES {
            assert_eq!(Dpi::new(*given).ok().map(Dpi::get), *want, "{name}");
        }
    }

    #[test]
    fn the_named_resolutions_are_valid_and_ordered() {
        for dpi in [Dpi::PAGE, Dpi::SCREEN, Dpi::PRINT] {
            assert_eq!(Dpi::new(dpi.get()), Ok(dpi));
        }
        assert!(Dpi::PAGE < Dpi::SCREEN && Dpi::SCREEN < Dpi::PRINT);
    }

    #[test]
    fn dpi_round_trips_and_refuses_a_bad_value_on_load() {
        assert_eq!(serde_json::to_string(&Dpi::PRINT).unwrap(), "300");
        assert_eq!(serde_json::from_str::<Dpi>("300").unwrap(), Dpi::PRINT);
        assert!(serde_json::from_str::<Dpi>("0").is_err());
    }
}
