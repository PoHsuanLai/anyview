//! The panel's states, tabs and the set of tabs a format has.

use ds_core::word::Word;

/// A tab of the side panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum PanelTab {
    /// Page or frame thumbnails.
    Thumbnails,
    /// The outline, chapters or headings.
    Contents,
    /// The file's facts.
    Info,
    /// Audio, subtitle and video tracks.
    Tracks,
}

impl PanelTab {
    const fn bit(self) -> u8 {
        match self {
            PanelTab::Thumbnails => 1,
            PanelTab::Contents => 2,
            PanelTab::Info => 4,
            PanelTab::Tracks => 8,
        }
    }
}

/// The tabs a format has, possibly none. The first in declaration order is the one a bare
/// toggle opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PanelTabs(u8);

impl PanelTabs {
    /// A format with no panel.
    pub const NONE: PanelTabs = PanelTabs(0);

    /// The set of `tabs`.
    pub const fn of(tabs: &[PanelTab]) -> Self {
        let mut bits = 0;
        let mut index = 0;
        while index < tabs.len() {
            bits |= tabs[index].bit();
            index += 1;
        }
        PanelTabs(bits)
    }

    /// Whether `tab` is in the set.
    pub const fn contains(self, tab: PanelTab) -> bool {
        self.0 & tab.bit() != 0
    }

    /// The first tab of the set, or `None` when the format has no panel.
    pub fn first(self) -> Option<PanelTab> {
        PanelTab::ALL
            .iter()
            .copied()
            .find(|tab| self.contains(*tab))
    }
}

/// Whether the panel is open, and on which tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Panel {
    /// Not showing.
    #[default]
    Hidden,
    /// Showing `tab`.
    Shown { tab: PanelTab },
}

/// What moves the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelIn {
    /// Open on the first tab, or close.
    Toggle,
    /// Show this tab, opening the panel if it is closed.
    Choose(PanelTab),
    /// Close.
    Close,
    /// The file changed, so the tabs in `PanelParams` may no longer include the shown one.
    TabsChanged,
    /// The clock; the panel keeps no timer.
    Elapsed,
}

impl From<ds_core::machine::Elapsed> for PanelIn {
    fn from(_: ds_core::machine::Elapsed) -> Self {
        PanelIn::Elapsed
    }
}

/// What the panel wants done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelOut {
    /// Show the panel on this tab.
    Show(PanelTab),
    /// Hide the panel.
    Hide,
}

/// What the open file has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PanelParams {
    /// The tabs this format offers.
    pub tabs: PanelTabs,
}
