//! What the viewer remembers of the left panel from one launch to the next.

/// Whether the left panel was open when the person last left it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PanelOpen {
    /// The panel was showing.
    Open,
    /// The panel was folded away.
    Closed,
}

/// The left panel as the person left it. One value for the person, not one per file.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PanelKept {
    /// Whether it was open.
    pub open: PanelOpen,
    /// The view it was last on, by its slug (`thumbnails`, `info`): a word this build does not
    /// know is ignored. `None` leaves the view already kept, which is how a close is kept.
    pub view: Option<String>,
}
