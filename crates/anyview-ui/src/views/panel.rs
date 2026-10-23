//! The sidebar: the body of a pane of the window's split view, never quire's `SidePanel`. Quire's
//! `Sidebar` with the file's views as the rows of its first section (Thumbnails, Contents, Info,
//! as Preview's sidebar lists them), and under them the chosen view: the open file's card or the
//! family's own list. Whether the pane is open and what it is on is the panel machine's; how wide
//! it is, is the split view's.

use crate::families::InfoCard;
use crate::{PanelTab, PanelTabs};
use anyview_core::{Facts, FormatKind};
use dioxus::prelude::*;
use ds::components::chrome::sidebar::Sidebar;
use ds::components::chrome::sidebar_model::{SidebarFill, SidebarSection};
use ds::prelude::{Icon, ListItem, Row, RowLeading, Selection};
use ds::style::tokens::control_size::SidebarSize;
use ds_core::vocab::RowState;
use ds_core::word::Word;

/// How wide the sidebar opens, in logical pixels, as mailo's and temor's do: a window that opens
/// with it shown is this much wider than its content, so the sidebar does not squeeze it (Preview
/// widens its window by the sidebar).
pub(super) const WIDTH: u32 = 232;

/// The icon of a view's row.
fn icon_of(tab: PanelTab) -> Icon {
    match tab {
        PanelTab::Thumbnails => Icon::Grid,
        // TODO(icons): ListTree, once quire has it.
        PanelTab::Contents => Icon::FileText,
        PanelTab::Sheets => Icon::Columns,
        PanelTab::Info => Icon::Info,
        PanelTab::Tracks => Icon::Music,
    }
}

/// The sidebar: `tab` is the view it is on, `tabs` those the file has, and `name`, `kind` and
/// `facts` the file the Info view is the card of. `body` is the family's list for the other views.
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
    let rows: Vec<ListItem<PanelTab>> = PanelTab::ALL
        .iter()
        .copied()
        .filter(|candidate| tabs.contains(*candidate))
        .map(|view| {
            let selection = if view == tab {
                Selection::Selected
            } else {
                Selection::Unselected
            };
            ListItem::row(
                view,
                view.label(),
                rsx! {
                    Row {
                        title: view.label(),
                        leading: RowLeading::Icon(icon_of(view)),
                        state: RowState {
                            selection,
                            ..RowState::default()
                        },
                        onclick: move |_| onchoose.call(view),
                    }
                },
            )
        })
        .collect();
    let content = rsx! {
        div { class: "viewer-panel-view",
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
    };
    rsx! {
        div { class: "viewer-panel",
            Sidebar::<PanelTab> {
                label: "Sidebar",
                // The ladder's 32, mailo's and temor's.
                size: SidebarSize::Medium,
                fill: SidebarFill::Flat,
                sections: vec![SidebarSection::List(rows), SidebarSection::Custom(content)],
                cursor: Some(tab),
                onselect: move |picked: PanelTab| onchoose.call(picked),
            }
        }
    }
}
