//! The screen of a file that did not open: what is wrong in words a person reads, and what can
//! be done about it (show it in its folder).

use crate::{LoadFailure, PlatformAbilities};
use anyview_core::FileAction;
use dioxus::prelude::*;
use ds::components::overlays::empty_state::EmptyForm;
use ds::prelude::{Button, EmptyState, Icon};

/// What the screen offers besides its words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Offer {
    /// The file is there to be found: its folder may be shown.
    Reveal,
    /// Nothing but the words.
    Nothing,
}

impl Offer {
    /// What a failed file offers: its folder, where there is a file manager and the file is named.
    /// A file the viewer cannot show gets its words and nothing more; there is no list of other
    /// programs to hand it to.
    pub(super) fn of(named: bool, platform: PlatformAbilities) -> Offer {
        if named && platform.offers(FileAction::RevealInFolder) {
            Offer::Reveal
        } else {
            Offer::Nothing
        }
    }
}

/// The title and the line under it for a failure of `reason`, of the file called `name`.
pub(super) fn words(reason: LoadFailure, name: &str) -> (&'static str, String) {
    match reason {
        LoadFailure::NotFound => (
            "This file was moved or deleted",
            if name.is_empty() {
                "It is no longer where it was.".to_owned()
            } else {
                format!("\u{201c}{name}\u{201d} is no longer where it was.")
            },
        ),
        LoadFailure::Unreadable => (
            "This file can\u{2019}t be read",
            "You may not have permission to open it.".to_owned(),
        ),
        LoadFailure::Unsupported => (
            "This file can\u{2019}t be shown here",
            "Another app may be able to open it.".to_owned(),
        ),
        LoadFailure::Damaged => (
            "This file looks damaged",
            "It may be incomplete or corrupted.".to_owned(),
        ),
        LoadFailure::Empty => (
            "This file is empty",
            "There is nothing in it to show.".to_owned(),
        ),
        LoadFailure::TooLarge => (
            "This file is too large to open here",
            "Another app may be able to open it.".to_owned(),
        ),
        LoadFailure::Locked => (
            "This file is protected",
            "It needs a password, and another app may be able to ask for it.".to_owned(),
        ),
    }
}

/// A file that did not open: the reason, and the buttons `offer` allows.
#[component]
pub(super) fn FailedScreen(
    reason: LoadFailure,
    name: String,
    offer: Offer,
    onreveal: EventHandler<()>,
) -> Element {
    let (title, description) = words(reason, &name);
    let action = match offer {
        Offer::Nothing => None,
        Offer::Reveal => Some(rsx! {
            div { class: "viewer-failed-actions",
                Button { label: "Show in Folder", onclick: move |_| onreveal.call(()) }
            }
        }),
    };
    rsx! {
        div { class: "viewer-failed",
            EmptyState {
                form: EmptyForm::Failure,
                title,
                description: Some(description.into()),
                icon: Some(Icon::TriangleAlert),
                action,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_failure_has_a_human_title_and_none_says_unsupported_for_a_large_file() {
        // name, reason, the words the title must hold
        const CASES: &[(&str, LoadFailure, &str)] = &[
            ("gone", LoadFailure::NotFound, "moved or deleted"),
            (
                "no permission",
                LoadFailure::Unreadable,
                "can\u{2019}t be read",
            ),
            (
                "not a thing",
                LoadFailure::Unsupported,
                "can\u{2019}t be shown",
            ),
            ("broken", LoadFailure::Damaged, "damaged"),
            ("nothing in it", LoadFailure::Empty, "empty"),
            ("big", LoadFailure::TooLarge, "too large"),
            ("password", LoadFailure::Locked, "protected"),
        ];
        for (name, reason, wanted) in CASES {
            let (title, _) = words(*reason, "a.json");
            assert!(title.contains(wanted), "{name}: {title}");
            assert!(!title.contains("Unsupported"), "{name}: {title}");
        }
    }
}
