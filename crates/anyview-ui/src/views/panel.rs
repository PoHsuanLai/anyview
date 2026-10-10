//! The left panel: the body of a pane of the window's split view, never quire's `SidePanel`. Tabs
//! (a segmented control, drawn only when the file has more than one) over the open file's card or
//! the family's own list. Whether the pane is open and what it is on is the panel machine's; how
//! wide it is, is the split view's.

use crate::families::InfoCard;
use crate::{PanelTab, PanelTabs};
use anyview_core::{Facts, FormatKind};
use dioxus::prelude::*;
use ds::components::controls::segmented::Tracking;
use ds::prelude::{Choice, SegmentedControl};
use ds_core::word::Word;

/// The panel: `tab` is the tab it is on, `tabs` those the file has, and `name`, `kind` and `facts`
/// the file the Info tab is the card of. `body` is the family's list for the other tabs.
#[component]
pub(super) fn InfoPanel(
    tab: PanelTab,
    tabs: PanelTabs,
    name: String,
    kind: Option<FormatKind>,
    facts: Facts,
    body: Option<Element>,
    onchoose: EventHandler<PanelTab>,
) -> Element {
    let choices: Vec<PanelTab> = PanelTab::ALL
        .iter()
        .copied()
        .filter(|candidate| tabs.contains(*candidate))
        .collect();
    rsx! {
        div { class: "viewer-panel", role: "complementary", "aria-label": "Info",
            if choices.len() > 1 {
                div { class: "viewer-panel-tabs",
                    SegmentedControl::<PanelTab> {
                        label: "Panel tab",
                        choices: choices.iter().map(|choice| Choice::new(*choice, choice.label())).collect::<Vec<_>>(),
                        tracking: Tracking::SelectOne(tab),
                        onchange: move |picked: PanelTab| onchoose.call(picked),
                    }
                }
            }
            match tab {
                PanelTab::Info => rsx! {
                    InfoCard { name, kind, facts }
                },
                PanelTab::Thumbnails | PanelTab::Contents | PanelTab::Sheets | PanelTab::Tracks => {
                    rsx! {}
                }
            }
            if let Some(body) = body {
                {body}
            }
        }
    }
}
