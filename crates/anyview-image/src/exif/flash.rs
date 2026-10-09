//! What the EXIF flash field says: whether the flash fired and in what mode.

/// Whether the flash lit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlashState {
    /// The flash fired.
    Fired,
    /// It did not.
    NotFired,
    /// The camera has no flash.
    Absent,
}

/// How the camera was told to use the flash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlashMode {
    /// The camera did not say.
    Unknown,
    /// Forced on.
    On,
    /// Forced off.
    Off,
    /// The camera decided.
    Auto,
}

/// The flash of a photo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Flash {
    /// Whether it fired.
    pub state: FlashState,
    /// The mode the camera was in.
    pub mode: FlashMode,
}

impl Flash {
    /// The flash the EXIF field value `field` describes: bit 0 is fired, bits 3 and 4 the mode,
    /// and bit 5 says the camera has no flash function at all.
    pub(super) fn of_field(field: u32) -> Self {
        Flash {
            state: if field & 0x20 != 0 {
                FlashState::Absent
            } else if field & 1 == 1 {
                FlashState::Fired
            } else {
                FlashState::NotFired
            },
            mode: match field >> 3 & 3 {
                1 => FlashMode::On,
                2 => FlashMode::Off,
                3 => FlashMode::Auto,
                _ => FlashMode::Unknown,
            },
        }
    }

    /// `Fired`, `Did not fire`, `Auto, fired`, `Off, did not fire`, `No flash`.
    pub fn text(self) -> String {
        let state = match self.state {
            FlashState::Absent => return "No flash".to_owned(),
            FlashState::Fired => "fired",
            FlashState::NotFired => "did not fire",
        };
        let mode = match self.mode {
            FlashMode::Unknown => return capitalised(state),
            FlashMode::On => "On",
            FlashMode::Off => "Off",
            FlashMode::Auto => "Auto",
        };
        format!("{mode}, {state}")
    }
}

fn capitalised(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flash_fields_read_as_words() {
        // name, field, words
        const CASES: &[(&str, u32, &str)] = &[
            ("fired", 0x01, "Fired"),
            ("not fired", 0x00, "Did not fire"),
            ("auto fired", 0x19, "Auto, fired"),
            ("auto not fired", 0x18, "Auto, did not fire"),
            ("forced on", 0x09, "On, fired"),
            ("forced off", 0x10, "Off, did not fire"),
            ("no flash function", 0x20, "No flash"),
            ("no flash function, off", 0x30, "No flash"),
            (
                "fired with red-eye reduction and no mode bits",
                0x41,
                "Fired",
            ),
        ];
        for (name, field, want) in CASES {
            assert_eq!(Flash::of_field(*field).text(), *want, "{name}");
        }
    }
}
