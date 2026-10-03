//! The side panel, hidden until asked for: tabs over the open file's facts. What the panel shows
//! is the panel machine's state; this draws it.

use crate::{PanelTab, PanelTabs};
use anyview_core::Facts;
use dioxus::prelude::*;
use ds::components::controls::segmented::Tracking;
use ds::components::fields::fact_list::{Fact, FactList};
use ds::prelude::{Choice, SegmentedControl, Shown, SidePanel};
use ds_core::word::Word;

/// The rows of `facts` as the panel lists them.
fn rows(facts: &Facts) -> Vec<Fact> {
    facts
        .rows()
        .iter()
        .map(|row| Fact::new(row.label.label(), row.value.as_str()))
        .collect()
}

/// The panel: `shown` is the machine's, `tab` the tab it is on, `tabs` those the file has.
#[component]
pub(super) fn InfoPanel(
    shown: Shown,
    tab: PanelTab,
    tabs: PanelTabs,
    facts: Facts,
    body: Option<Element>,
    onchoose: EventHandler<PanelTab>,
    onclose: EventHandler<()>,
) -> Element {
    let choices: Vec<PanelTab> = PanelTab::ALL
        .iter()
        .copied()
        .filter(|candidate| tabs.contains(*candidate))
        .collect();
    rsx! {
        SidePanel {
            label: "Info",
            shown,
            onclose,
            header: rsx! {
                if choices.len() > 1 {
                    SegmentedControl::<PanelTab> {
                        label: "Panel tab",
                        choices: choices.iter().map(|choice| Choice::new(*choice, choice.label())).collect::<Vec<_>>(),
                        tracking: Tracking::SelectOne(tab),
                        onchange: move |picked: PanelTab| onchoose.call(picked),
                    }
                }
            },
            match tab {
                PanelTab::Info => rsx! { FactList { facts: rows(&facts) } },
                PanelTab::Thumbnails | PanelTab::Contents | PanelTab::Tracks => rsx! {},
            }
            if let Some(body) = body {
                {body}
            }
        }
    }
}
