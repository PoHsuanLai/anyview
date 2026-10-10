//! The root of a viewer window: what it was opened with, its stylesheet, and the `Ds` root that
//! the window (`window.rs`) is drawn in. The look is Proposal 1: only the content shows by
//! default, the titlebar and the capsule come with the pointer, ⌘K opens the palette and the
//! side panel waits to be asked for.

use super::pane::PaneSeat;
use super::window::ViewerWindow;
use crate::{Look, LookFeed, Presentation};
use anyview_core::{FilePath, Sequence};
use dioxus::prelude::*;
use ds::prelude::*;
use ds_core::vocab::{Activity, InputModality};

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

/// The root of a viewer pane: the stylesheet and the window, drawn as a region of the host's
/// window. The host provides the same contexts as for a window (`Launch` with
/// `Presentation::Pane`, and the `Edge`), and a `PaneSeat`. With a `LookFeed` the pane is a `Ds`
/// root of its own and follows the feed; with none it takes the host's tokens and follows the host.
#[component]
pub fn PaneApp() -> Element {
    let launch = use_hook(consume_context::<Launch>);
    let look = use_look(launch.look.clone());
    let followed = use_hook(|| try_consume_context::<LookFeed>().is_some());
    let now = look();
    let window = rsx! {
        AppStyle { css: stylesheet() }
        ViewerWindow { launch: launch.clone() }
    };
    if followed {
        rsx! {
            PaneSignals {
                Ds {
                    appearance: now.appearance,
                    system: now.system,
                    tint_alpha: now.tint_alpha,
                    typeface: now.typeface,
                    stack: now.stack,
                    material: Material::Window,
                    {window}
                }
            }
        }
    } else {
        window
    }
}

/// The pane's own `Ds` root draws active while the host says the pane has the keyboard, and
/// inactive (selection and caret at rest) while it does not, as a window does that lost key status.
/// The host's modality and scale are shared as they are; only the activity is the pane's own, so
/// the host's window is not marked inactive by it.
#[component]
fn PaneSignals(children: Element) -> Element {
    let seat = use_hook(try_consume_context::<PaneSeat>);
    let host = use_hook(try_consume_context::<HostSignals>);
    let own_modality = use_signal(InputModality::default);
    let own_scale = use_signal(|| Scale::ONE);
    let mut activity = use_signal(|| seat.map_or(Activity::Active, activity_of));
    use_effect(move || {
        let next = seat.map_or(Activity::Active, activity_of);
        if *activity.peek() != next {
            activity.set(next);
        }
    });
    use_context_provider(|| HostSignals {
        modality: host.map_or(own_modality, |host| host.modality),
        scale: host.map_or(own_scale, |host| host.scale),
        activity,
    });
    children
}

/// Whether the host has given the pane the keyboard, as the activity of a window.
fn activity_of(seat: PaneSeat) -> Activity {
    if (seat.focused)() {
        Activity::Active
    } else {
        Activity::Inactive
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
