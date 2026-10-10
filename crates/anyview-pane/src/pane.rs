//! The pane component: the viewer's window over an edge of the host's, seated in the host's focus.

use crate::edge::PaneEdge;
use crate::handle::PaneHandle;
use crate::request::PaneRequest;
use crate::store::Kept;
use anyview_core::{FilePath, Sequence};
use anyview_ui::{
    Edge, HostRequest, Launch, Look, LookFeed, PaneApp, PaneChrome, PaneSeat, Presentation,
    use_pane_link,
};
use dioxus::prelude::*;
use futures_channel::mpsc::{UnboundedReceiver, unbounded};
use futures_util::StreamExt;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

/// What a host gives [`ViewerPane`].
#[derive(Props, Clone, PartialEq)]
pub struct PaneProps {
    /// The file to show, an absolute path. A different file replaces the pane's (the host owns drag
    /// and drop, and sends the new file); walking the `sequence` with the arrow keys does not
    /// change it.
    pub file: FilePath,
    /// The files the arrow keys walk, with `file`'s place among them; none for a single file.
    #[props(default)]
    pub sequence: Option<Sequence>,
    /// The workers and the seams the pane runs over.
    pub edge: PaneEdge,
    /// Whether the host has given the pane the keyboard. While it is false the pane handles no key
    /// and consumes none; when it becomes true the pane takes the keyboard focus.
    pub focused: ReadSignal<bool>,
    /// What the pane asks of its host.
    pub on_request: EventHandler<PaneRequest>,
    /// The handle the host reads the pane's commands from and runs them through
    /// ([`use_pane_handle`](crate::use_pane_handle)); none when the host lists no commands.
    #[props(default)]
    pub handle: Option<PaneHandle>,
    /// The desktop's look as it changes. With none the pane follows the host's own tokens.
    #[props(default)]
    pub look: Option<LookFeed>,
    /// How much of the viewer's own chrome the pane draws.
    #[props(default)]
    pub chrome: PaneChrome,
}

impl std::fmt::Debug for PaneProps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The handlers and signals have nothing to show.
        f.debug_struct("PaneProps")
            .field("file", &self.file)
            .field("edge", &self.edge)
            .field("chrome", &self.chrome)
            .finish_non_exhaustive()
    }
}

/// The viewer in a region of the host's window.
///
/// - **Drawn:** the file, the capsule of controls when the pointer is over the pane, the info
///   panel (a layer over the file when the pane is narrow) and the context menu.
/// - **Not drawn:** a titlebar or traffic lights, the welcome window, sheets, the palette, file
///   drop. Sheets are the host's: a rename or a trash is a [`PaneRequest::File`].
/// - **Keys:** plain keys only, and only while `focused`: the arrows walk the sequence, Space,
///   PageUp/PageDown, Home/End and `+`/`-`/`0` move through the file. Every Ctrl, Alt or Super chord
///   is left to the host. Esc undoes the innermost thing (a find, a scrub, the context menu, the
///   panel) and then asks to give the keyboard back ([`PaneRequest::Unfocus`]); it never closes the
///   pane.
#[component]
pub fn ViewerPane(props: PaneProps) -> Element {
    // A different file is a different pane: it opens at its own place, with its own edge and a
    // fresh machine, the way a window made for it would.
    let key = props.file.as_path().display().to_string();
    rsx! {
        Seated {
            key: "{key}",
            file: props.file.clone(),
            sequence: props.sequence.clone(),
            edge: props.edge.clone(),
            focused: props.focused,
            on_request: props.on_request,
            handle: props.handle,
            look: props.look.clone(),
            chrome: props.chrome,
        }
    }
}

/// The pane for one file: its contexts, and the requests of its edge on their way to the host.
#[component]
fn Seated(props: PaneProps) -> Element {
    let own_link = use_pane_link();
    let link = props.handle.map_or(own_link, |handle| handle.link());
    // The host's handler as of the latest render: a request is answered by the closure the host
    // gave last, not the one it gave when the pane opened.
    let answer = use_hook(|| Rc::new(Cell::new(props.on_request)));
    answer.set(props.on_request);

    let (edge, inbox) = use_hook(|| {
        let (sender, receiver) = unbounded::<HostRequest>();
        let mut services = props.edge.services();
        services.on_request = Arc::new(move |request| {
            // The pane is gone when the receiver is: there is nobody to ask.
            let _gone = sender.unbounded_send(request);
        });
        (Edge::new(services), Rc::new(RefCell::new(Some(receiver))))
    });
    let heard = answer.clone();
    let writing = edge.clone();
    use_future(move || {
        let taken: Option<UnboundedReceiver<HostRequest>> = inbox.borrow_mut().take();
        let answer = heard.clone();
        let edge = writing.clone();
        async move {
            let Some(mut requests) = taken else { return };
            let mut kept = Kept::default();
            while let Some(request) = requests.next().await {
                // With a store, the pane keeps the views and places itself, on the workers.
                if kept.answers(&request, &edge) {
                    continue;
                }
                if let Some(request) = PaneRequest::from_host(request) {
                    answer.get().call(request);
                }
            }
        }
    });

    // Esc has given the keyboard back; the host may take a while to say so, and the focus leaving
    // the pane in the meantime is the same loss, not a second one.
    let asked = use_hook(|| Rc::new(Cell::new(false)));
    let unfocus = {
        let (answer, asked) = (answer.clone(), asked.clone());
        use_callback(move |(): ()| {
            asked.set(true);
            answer.get().call(PaneRequest::Unfocus);
        })
    };
    let lost = {
        let (answer, asked) = (answer.clone(), asked.clone());
        use_callback(move |(): ()| {
            if !asked.replace(true) {
                answer.get().call(PaneRequest::Unfocus);
            }
        })
    };
    // Given the keyboard again, the pane may ask to give it back again.
    let given = props.focused;
    use_effect({
        let asked = asked.clone();
        move || {
            if given() {
                asked.set(false);
            }
        }
    });
    let (file, sequence) = (props.file.clone(), props.sequence.clone());
    use_context_provider(|| Launch {
        file,
        sequence,
        look: Look::default(),
        presentation: Presentation::Pane,
    });
    use_context_provider(|| edge.clone());
    let (focused, chrome) = (props.focused, props.chrome);
    use_context_provider(|| {
        PaneSeat::new(focused, link, unfocus)
            .with_focus_lost(lost)
            .with_chrome(chrome)
    });

    match props.look.clone() {
        Some(feed) => rsx! {
            Fed { feed, PaneApp {} }
        },
        None => rsx! {
            PaneApp {}
        },
    }
}

/// The pane under the look feed the host gave it.
#[component]
fn Fed(feed: LookFeed, children: Element) -> Element {
    use_context_provider(|| feed.clone());
    children
}
