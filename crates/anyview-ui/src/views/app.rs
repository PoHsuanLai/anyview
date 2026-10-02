//! The root of a viewer window: what it was opened with, its stylesheet, and the `Ds` root that
//! the window (`window.rs`) is drawn in. The look is Proposal 1: only the content shows by
//! default, the titlebar and the capsule come with the pointer, ⌘K opens the palette and the
//! side panel waits to be asked for.

use super::window::ViewerWindow;
use crate::Presentation;
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
    /// How the window looks: theme, accent, motion.
    pub appearance: Appearance,
    /// How the window is on screen: a window, or the small borderless one of a recording.
    pub presentation: Presentation,
}

/// The viewer's own stylesheet, in the `app` layer; tokens only.
pub fn stylesheet() -> String {
    [
        include_str!("style.css"),
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
    rsx! {
        Ds { appearance: launch.appearance, material: Material::Window,
            AppStyle { css: stylesheet() }
            ViewerWindow { launch: launch.clone() }
        }
    }
}
