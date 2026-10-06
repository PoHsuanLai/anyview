//! The context menu over the window: quire's menu, placed at the point the machine holds, listing
//! the rows the machine's params name. A pick goes to the machine, which runs it; Esc and a click
//! outside close the menu through its own handling, and the machine hears of it as a close.

use crate::{ContextEntry, ContextPick, Spot};
use dioxus::prelude::*;
use ds::components::menus::item::item::MenuItem;
use ds::host::measure::Anchor;
use ds::prelude::{Menu, MenuPlacement, Point, Px};

/// The menu's lines as quire draws them.
pub(super) fn items_of(entries: &[ContextEntry]) -> Vec<MenuItem<ContextPick>> {
    entries
        .iter()
        .map(|entry| match entry {
            ContextEntry::Item { pick, title } => MenuItem::new(*pick, *title),
            ContextEntry::Separator => MenuItem::Separator,
        })
        .collect()
}

/// The context menu, open with its corner at `at`. A menu at another point is a new menu, so the
/// component is keyed by its place.
#[component]
pub(super) fn ContextPopup(
    entries: Vec<ContextEntry>,
    at: Spot,
    onpick: EventHandler<ContextPick>,
    onclose: EventHandler<()>,
) -> Element {
    let anchor = Anchor::Point(Point {
        x: Px(at.x as f32),
        y: Px(at.y as f32),
    });
    rsx! {
        Menu::<ContextPick> {
            key: "{at.x},{at.y}",
            placement: MenuPlacement::Context,
            anchor,
            items: items_of(&entries),
            onpick: move |pick| onpick.call(pick),
            onclose: move |()| onclose.call(()),
        }
    }
}
