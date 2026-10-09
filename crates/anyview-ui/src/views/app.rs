//! The root of a viewer window: what it was opened with, its stylesheet, and the `Ds` root that
//! the window (`window.rs`) is drawn in. The look is Proposal 1: only the content shows by
//! default, the titlebar and the capsule come with the pointer, ⌘K opens the palette and the
//! side panel waits to be asked for.

use super::window::ViewerWindow;
use crate::{Look, LookFeed, Presentation};
use anyview_core::{FilePath, Sequence};
use dioxus::prelude::*;
use ds::prelude::*;

/// What the window was opened with: the file, and the list the arrow keys walk, when there is one.
/// The binary provides it as a root context (`ds_blitz::AppConfig::with_context`).
#[derive(Debug, Clone, PartialEq)]
pub struct Launch {
    /// The file to open.
    pub file: FilePath,
    /// The files around it, with the file's place among them.
    pub sequence: Option<Sequence>,
    /// How the window looks when it opens (and for good, when no `LookFeed` is provided).
    pub look: Look,
    /// How the window is on screen: a window, or the small borderless one of a recording.
    pub presentation: Presentation,
}

/// The viewer's own stylesheet, in the `app` layer; tokens only.
pub fn stylesheet() -> String {
    [
        include_str!("window.css"),
        include_str!("peek.css"),
        include_str!("sheets.css"),
        include_str!("welcome.css"),
        crate::families::CARD_CSS,
        crate::families::RASTER_CSS,
        crate::families::TEXT_CSS,
        crate::families::DATA_CSS,
        crate::families::TOKEN_CSS,
        crate::families::PDF_CSS,
        crate::families::MEDIA_CSS,
    ]
    .join("\n")
}

/// The root of a viewer window: a quire `Ds` root, the stylesheet, and the window.
#[component]
pub fn ViewerApp() -> Element {
    let launch = use_hook(consume_context::<Launch>);
    let look = use_look(launch.look.clone());
    let now = look();
    rsx! {
        Ds {
            appearance: now.appearance,
            system: now.system,
            tint_alpha: now.tint_alpha,
            typeface: now.typeface,
            stack: now.stack,
            material: Material::Window,
            // The install sheet is the shell's, so its rules come with the shell's stylesheet.
            sheet: Some(ds_shell::stylesheet()),
            AppStyle { css: stylesheet() }
            ViewerWindow { launch: launch.clone() }
        }
    }
}

/// The look the window draws now: the feed's latest, which follows the desktop while the window is
/// open, or `launched`, the one it was opened with. The feed is read once, so every render takes one branch.
pub(super) fn use_look(launched: Look) -> ReadSignal<Look> {
    let feed = use_hook(try_consume_context::<LookFeed>);
    let mut look = use_signal({
        let feed = feed.clone();
        move || match feed {
            Some(LookFeed(receiver)) => receiver.borrow().clone(),
            None => launched,
        }
    });
    use_future(move || {
        let feed = feed.clone();
        async move {
            let Some(LookFeed(mut receiver)) = feed else {
                return;
            };
            while receiver.changed().await.is_ok() {
                let latest = receiver.borrow_and_update().clone();
                look.set(latest);
            }
        }
    });
    ReadSignal::new(look)
}
