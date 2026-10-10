//! The pointer tool: whether a drag on a zoomed picture pans it, or draws a crop. Pan is the
//! default, as in Preview, Photos and Loupe; Select is the other choice of Preview's tool control,
//! Crop is the third (a mode with its own rectangle, as in Photos), and holding Space pans for as
//! long as the key is down. The wheel and the touchpad pan whatever the tool says.

/// The tool, set by its control, its key or its palette row. It carries from file to file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Tool {
    /// A drag does not pan.
    Select,
    /// A drag pans.
    #[default]
    Pan,
    /// A drag moves the corners and edges of a crop rectangle.
    Crop,
}

impl Tool {
    /// The other of Select and Pan, which H switches between; Crop goes back to Pan.
    pub fn other(self) -> Tool {
        match self {
            Tool::Select | Tool::Crop => Tool::Pan,
            Tool::Pan => Tool::Select,
        }
    }

    /// The word the control and the palette name it by.
    pub fn label(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Pan => "Pan",
            Tool::Crop => "Crop",
        }
    }
}

/// Whether Space is held down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Space {
    /// Not held.
    #[default]
    Up,
    /// Held: the hand is out for as long as it is.
    Down,
}

/// The hand's whole state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Hand {
    /// The tool the person switched on or off.
    pub tool: Tool,
    /// Whether Space holds it out for now.
    pub space: Space,
}

/// What moves the hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandIn {
    /// H: the other tool.
    Toggle,
    /// The control or the palette's row: this tool.
    Use(Tool),
    /// Space went down on a picture.
    SpaceDown,
    /// Space came up, or the window lost the keyboard.
    SpaceUp,
}

impl Hand {
    /// Whether a drag pans now: the tool is on, or Space is held.
    pub fn pans(self) -> bool {
        self.tool == Tool::Pan || self.space == Space::Down
    }

    /// The hand after `input`.
    pub fn step(self, input: HandIn) -> Hand {
        match input {
            HandIn::Toggle => Hand {
                tool: self.tool.other(),
                ..self
            },
            HandIn::Use(tool) => Hand { tool, ..self },
            HandIn::SpaceDown => Hand {
                space: Space::Down,
                ..self
            },
            HandIn::SpaceUp => Hand {
                space: Space::Up,
                ..self
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pan_is_the_default_and_space_holds_the_hand_out_over_select() {
        // name, inputs, whether a drag pans, the tool
        const CASES: &[(&str, &[HandIn], bool, Tool)] = &[
            ("at rest", &[], true, Tool::Pan),
            ("toggled to select", &[HandIn::Toggle], false, Tool::Select),
            (
                "toggled twice",
                &[HandIn::Toggle, HandIn::Toggle],
                true,
                Tool::Pan,
            ),
            (
                "select chosen",
                &[HandIn::Use(Tool::Select)],
                false,
                Tool::Select,
            ),
            (
                "pan chosen twice stays pan",
                &[HandIn::Use(Tool::Pan), HandIn::Use(Tool::Pan)],
                true,
                Tool::Pan,
            ),
            (
                "space held over select",
                &[HandIn::Use(Tool::Select), HandIn::SpaceDown],
                true,
                Tool::Select,
            ),
            (
                "space released back to select",
                &[
                    HandIn::Use(Tool::Select),
                    HandIn::SpaceDown,
                    HandIn::SpaceUp,
                ],
                false,
                Tool::Select,
            ),
            (
                "crop chosen holds nothing out",
                &[HandIn::Use(Tool::Crop)],
                false,
                Tool::Crop,
            ),
            (
                "the other tool from crop is pan",
                &[HandIn::Use(Tool::Crop), HandIn::Toggle],
                true,
                Tool::Pan,
            ),
            (
                "space held over crop pans for as long as it is down",
                &[HandIn::Use(Tool::Crop), HandIn::SpaceDown],
                true,
                Tool::Crop,
            ),
            (
                "a repeated key is one hold",
                &[
                    HandIn::Use(Tool::Select),
                    HandIn::SpaceDown,
                    HandIn::SpaceDown,
                    HandIn::SpaceUp,
                ],
                false,
                Tool::Select,
            ),
        ];
        for (name, inputs, pans, tool) in CASES {
            let hand = inputs.iter().fold(Hand::default(), |hand, i| hand.step(*i));
            assert_eq!(hand.pans(), *pans, "{name}");
            assert_eq!(hand.tool, *tool, "{name}");
        }
    }
}
