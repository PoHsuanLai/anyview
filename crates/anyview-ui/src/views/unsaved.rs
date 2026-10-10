//! The question asked before going on with changes that are not saved: Save, Don't Save and
//! Cancel, as a Mac asks it of a document. A text editor and a picture editor ask it the same way,
//! so it knows nothing of what the changes are.

use dioxus::prelude::*;
use ds::components::overlays::alert_model::{AlertButton, AlertRole};
use ds::prelude::Alert;

/// "Do you want to save the changes made to “name”?": Save is the default (Return), Cancel
/// answers Escape, and Don't Save is never the default, so a reflexive Return loses nothing.
#[component]
pub(super) fn UnsavedSheet(
    name: String,
    onsave: EventHandler<()>,
    ondiscard: EventHandler<()>,
    oncancel: EventHandler<()>,
) -> Element {
    rsx! {
        Alert {
            title: format!("Do you want to save the changes made to \u{201c}{name}\u{201d}?"),
            message: Some("Your changes will be lost if you don\u{2019}t save them.".into()),
            buttons: vec![
                AlertButton::new("Save", AlertRole::Normal, onsave),
                AlertButton::new("Cancel", AlertRole::Cancel, oncancel),
                AlertButton::new("Don\u{2019}t Save", AlertRole::Destructive, ondiscard),
            ],
        }
    }
}
