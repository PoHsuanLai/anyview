//! The window of a launch with no file: what a mature viewer shows instead of nothing. A line of
//! welcome, an Open button that asks the desktop's file dialog, and the window itself is a drop
//! target. What is chosen or dropped goes to the host as the files to open, which opens each in a
//! window of its own and closes this one.

use super::app::use_look;
use super::keys::shortcut_of;
use crate::io::{Done, HostRequest};
use crate::{Edge, Look, stylesheet};
use anyview_core::FilePath;
use dioxus::prelude::*;
use ds::components::chrome::titlebar_parts::TitleParts;
use ds::components::chrome::window_frame::{TrafficLights, WindowTitlebar};
use ds::components::controls::button_model::Answers;
use ds::file_drop::hook::use_file_drop;
use ds::focus::soon::focus_soon;
use ds::prelude::*;
use ds_core::vocab::ShortcutKey;
use futures_util::StreamExt;

/// The root of the welcome window: a quire `Ds` root, the stylesheet and the welcome. It reads its
/// `Edge` from the context the binary provides, as the viewer's does.
#[component]
pub fn WelcomeApp() -> Element {
    let look = use_look(Look::default());
    let now = look();
    rsx! {
        Ds {
            appearance: now.appearance,
            system: now.system,
            tint_alpha: now.tint_alpha,
            typeface: now.typeface,
            stack: now.stack,
            material: Material::Window,
            AppStyle { css: stylesheet() }
            Welcome {}
        }
    }
}

#[component]
fn Welcome() -> Element {
    let edge = use_hook(consume_context::<Edge>);
    let toasts = use_toasts();
    // The file dialog's answer, and the host's notices, come back through the mailbox.
    let mailbox = edge.clone();
    use_future(move || {
        let taken = mailbox.take_mailbox();
        let edge = mailbox.clone();
        async move {
            let Some(mut inbox) = taken else { return };
            while let Some(done) = inbox.next().await {
                if let Done::Chosen { files } = done {
                    edge.request(HostRequest::OpenFiles(files));
                } else if let Done::Notice(notice) = done {
                    toasts.push(notice.text, None);
                }
            }
        }
    });
    let dropped = edge.clone();
    let drop = use_file_drop(move |files: ds::file_drop::drag::FileDrop| {
        let paths: Vec<FilePath> = files
            .paths
            .iter()
            .filter_map(|path| FilePath::new(path).ok())
            .collect();
        if !paths.is_empty() {
            dropped.request(HostRequest::OpenFiles(paths));
        }
    });
    let (pick, key) = (edge.clone(), edge);
    rsx! {
        div {
            class: "viewer-welcome",
            tabindex: "0",
            "data-drop": drop.drop_attr(),
            onmounted: move |event| {
                focus_soon(event.data());
                drop.mounted(event);
            },
            onkeydown: move |event: KeyboardEvent| {
                // ⌘O opens the dialog, ⌘W closes the window.
                let keys = shortcut_of(&event)
                    .map(|key| key.keys())
                    .unwrap_or_default();
                match keys.as_slice() {
                    [ShortcutKey::Super, ShortcutKey::Char('o')] => {
                        event.prevent_default();
                        key.request(HostRequest::PickFile);
                    }
                    [ShortcutKey::Super, ShortcutKey::Char('w')] => {
                        event.prevent_default();
                        key.request(HostRequest::CloseWindow);
                    }
                    _ => {}
                }
            },
            WindowTitlebar {
                title: "Anyview".to_owned(),
                parts: TitleParts::default(),
                lights: TrafficLights::Shown,
            }
            div { class: "viewer-welcome-body",
                EmptyState {
                    title: "Open a file to view it",
                    description: Some("Choose a picture, a document or a recording, or drop one on this window.".into()),
                    icon: Some(Icon::Image),
                    action: rsx! {
                        Button {
                            label: "Open\u{2026}",
                            answers: Answers::Return,
                            onclick: move |_| pick.request(HostRequest::PickFile),
                        }
                    },
                }
            }
        }
    }
}
