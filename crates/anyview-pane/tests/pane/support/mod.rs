//! A pane under the harness: a host that draws one or more `ViewerPane`s in a row, workers that run
//! each job where it is submitted, a folder of real files for the arrow keys to walk, the requests
//! the panes made of their host and the keys that reached the host unconsumed.
#![allow(dead_code)]

use anyview_core::Source;
use anyview_pane::{
    FilePath, HitList, NonEmpty, PaneCommand, PaneEdge, PaneRequest, Sequence, SequenceOrigin,
    ViewerPane, Work, Workers, use_pane_handle,
};
use dioxus::prelude::*;
use ds::components::menus::palette::palette_group::{GroupEntries, PaletteGroup};
use ds::prelude::{Appearance, Ds, Material};
use ds_harness::{Backend, Clock, Driver, Harness, HarnessConfig, Input, Query, Viewport};
use futures_channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use futures_util::StreamExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Runs each job on the thread that submitted it, so a result is in the mailbox by the time the
/// harness looks, and counts them: panes that share a pool share the count.
#[derive(Debug, Default)]
pub struct Counting(AtomicUsize);

impl Counting {
    /// How many pieces of work every pane has submitted to this pool.
    pub fn submitted(&self) -> usize {
        self.0.load(Ordering::SeqCst)
    }
}

impl Workers for Counting {
    fn submit(&self, work: Work) {
        self.0.fetch_add(1, Ordering::SeqCst);
        work.run();
    }
}

/// A key that reached the host's own handler (it bubbled out of the pane), and whether the pane
/// took it (prevented its default).
#[derive(Debug, Clone, PartialEq)]
pub struct Heard {
    pub key: Key,
    pub taken: bool,
}

/// What the host's palette asks of the first pane's handle.
#[derive(Debug, Clone, PartialEq)]
pub enum Call {
    Find(String),
    EndFind,
    Run(PaneCommand),
}

/// The rows the host's palette would list for the find that is up: the first few hits with a
/// Show All row, and every hit. Each is the value picking the row yields.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Listed {
    pub brief: Vec<PaneCommand>,
    pub whole: Vec<PaneCommand>,
}

fn values(group: PaletteGroup<PaneCommand>) -> Vec<PaneCommand> {
    match group.entries {
        GroupEntries::List(rows) => rows.into_iter().map(|row| row.value).collect(),
        GroupEntries::Grid(_) => Vec::new(),
    }
}

/// What the host under test holds and the test reads.
#[derive(Clone)]
pub struct Rig {
    calls: Arc<Mutex<Option<UnboundedReceiver<Call>>>>,
    listed: Arc<Mutex<Listed>>,
    panes: Vec<(FilePath, Option<Sequence>)>,
    edge: PaneEdge,
    requests: Arc<Mutex<Vec<PaneRequest>>>,
    heard: Arc<Mutex<Vec<Heard>>>,
    focus: Arc<Mutex<Option<UnboundedReceiver<bool>>>>,
}

/// A host under the harness and what the test holds of it.
pub struct Host {
    pub harness: Harness,
    /// What the panes asked of the host, oldest first.
    pub requests: Arc<Mutex<Vec<PaneRequest>>>,
    /// The keys that bubbled out of a pane to the host.
    pub heard: Arc<Mutex<Vec<Heard>>>,
    /// The pool every pane shares.
    pub workers: Arc<Counting>,
    focus: UnboundedSender<bool>,
    calls: UnboundedSender<Call>,
    listed: Arc<Mutex<Listed>>,
}

impl Host {
    /// Ask the first pane's handle for something, as the host's palette would.
    pub fn ask(&mut self, call: Call) {
        self.calls.unbounded_send(call).unwrap();
        self.settle();
    }

    /// Feed the first pane's find a query typed in the host's palette, and let the search answer.
    pub fn find(&mut self, query: &str) {
        self.ask(Call::Find(query.to_owned()));
        self.settle();
    }

    /// The hit rows the host's palette lists now.
    pub fn listed(&self) -> Listed {
        self.listed.lock().unwrap().clone()
    }

    /// The number of the first line the first pane shows.
    pub fn first_line(&self) -> Option<u32> {
        self.harness.text_of(".viewer-lineno")?.trim().parse().ok()
    }

    /// Give the panes the keyboard, or take it back.
    pub fn focus(&mut self, focused: bool) {
        self.focus.unbounded_send(focused).unwrap();
        self.settle();
    }

    pub fn settle(&mut self) {
        self.harness.advance(Duration::from_millis(300));
    }

    /// The files the panes have said they show, oldest first.
    pub fn shown(&self) -> Vec<String> {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter_map(|request| {
                let PaneRequest::Opened(opened) = request else {
                    return None;
                };
                opened
                    .source
                    .path()
                    .file_name()
                    .map(|name| name.as_str().to_owned())
            })
            .collect()
    }

    /// The last file a pane said it shows.
    pub fn showing(&self) -> Option<String> {
        self.shown().pop()
    }

    /// Whether any pane asked for this.
    pub fn asked(&self, wanted: &PaneRequest) -> bool {
        self.times(wanted) > 0
    }

    /// How many times the panes asked for this.
    pub fn times(&self, wanted: &PaneRequest) -> usize {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .filter(|request| *request == wanted)
            .count()
    }

    /// Whether any pane asked the host to keep where the person is.
    pub fn asked_to_remember(&self) -> bool {
        self.requests.lock().unwrap().iter().any(|request| {
            let PaneRequest::Remember(_) = request else {
                return false;
            };
            true
        })
    }

    /// The file the panes last said they show, as the viewer probed it.
    pub fn source(&self) -> Source {
        self.requests
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find_map(|request| {
                let PaneRequest::Opened(opened) = request else {
                    return None;
                };
                Some(opened.source.clone())
            })
            .expect("a pane showed a file")
    }

    /// Click the host's own focusable thing beside the panes, which takes the keyboard focus from
    /// whichever pane had it.
    pub fn click_elsewhere(&mut self) {
        let at = self
            .harness
            .rect("#elsewhere")
            .expect("the host's own spot");
        self.harness.send(Input::click(at.origin));
        self.settle();
    }
}

#[allow(non_snake_case)]
fn Row() -> Element {
    let rig = use_hook(consume_context::<Rig>);
    let mut focused = use_signal(|| true);
    let changes = rig.focus.clone();
    use_future(move || {
        let taken = changes.lock().unwrap().take();
        async move {
            let Some(mut changes) = taken else { return };
            while let Some(now) = changes.next().await {
                focused.set(now);
            }
        }
    });
    let handle = use_pane_handle();
    let calls = rig.calls.clone();
    use_future(move || {
        let taken = calls.lock().unwrap().take();
        async move {
            let Some(mut calls) = taken else { return };
            while let Some(call) = calls.next().await {
                match call {
                    Call::Find(query) => handle.find(query),
                    Call::EndFind => handle.end_find(),
                    Call::Run(command) => handle.run(command),
                }
            }
        }
    });
    // Reading the hits subscribes the host, as its palette would be.
    *rig.listed.lock().unwrap() = Listed {
        brief: values(handle.hits(HitList::Brief)),
        whole: values(handle.hits(HitList::Whole)),
    };
    let heard = rig.heard.clone();
    let panes: Vec<Element> = rig
        .panes
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, (file, sequence))| {
            let requests = rig.requests.clone();
            rsx! {
                div { key: "{index}", style: "flex:1;min-width:0;height:100%;",
                    ViewerPane {
                        file,
                        sequence,
                        edge: rig.edge.clone(),
                        handle: (index == 0).then_some(handle),
                        focused: ReadSignal::new(focused),
                        on_request: move |request: PaneRequest| requests.lock().unwrap().push(request),
                    }
                }
            }
        })
        .collect();
    rsx! {
        Ds { appearance: Appearance::default(), material: Material::Window,
            div {
                style: "display:flex;width:100vw;height:100vh;",
                onkeydown: move |event: KeyboardEvent| {
                    heard.lock().unwrap().push(Heard {
                        key: event.key(),
                        taken: !event.default_action_enabled(),
                    });
                },
                {panes.into_iter()}
                // The host's own, outside every pane, over a corner of the last one.
                div {
                    id: "elsewhere",
                    tabindex: "0",
                    style: "position:fixed;right:0;bottom:0;width:20px;height:20px;",
                }
            }
        }
    }
}

/// The folder's files as the sequence the arrow keys walk, opened at `at`.
pub fn sequence_of(paths: &[PathBuf], at: usize) -> Sequence {
    let entries: Vec<FilePath> = paths
        .iter()
        .map(|path| FilePath::new(path).unwrap())
        .collect();
    let current = entries[at].clone();
    Sequence::starting_at(
        NonEmpty::from_vec(entries).unwrap(),
        &current,
        SequenceOrigin::Selection,
    )
    .unwrap()
}

/// One pane per entry of `panes`, side by side in a region of `viewport`, over one pool.
pub fn hosted(panes: Vec<(PathBuf, Option<Sequence>)>, viewport: Viewport) -> Host {
    hosted_over(panes, viewport, None)
}

/// The same, over the viewer's store at `store` when there is one.
pub fn hosted_over(
    panes: Vec<(PathBuf, Option<Sequence>)>,
    viewport: Viewport,
    store: Option<&Path>,
) -> Host {
    let workers = Arc::new(Counting::default());
    let edge = PaneEdge::portable(workers.clone());
    let edge = match store {
        Some(root) => edge.with_store(root),
        None => edge,
    };
    let (focus, changes) = unbounded();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let heard = Arc::new(Mutex::new(Vec::new()));
    let (calls, asked) = unbounded();
    let listed = Arc::new(Mutex::new(Listed::default()));
    let rig = Rig {
        calls: Arc::new(Mutex::new(Some(asked))),
        listed: listed.clone(),
        panes: panes
            .into_iter()
            .map(|(path, sequence)| (FilePath::new(path).unwrap(), sequence))
            .collect(),
        edge,
        requests: requests.clone(),
        heard: heard.clone(),
        focus: Arc::new(Mutex::new(Some(changes))),
    };
    let config = HarnessConfig::new(viewport)
        .with_clock(Clock::Virtual)
        .with_backend(Backend::Hybrid)
        .with_context(rig);
    let mut harness = Harness::new(Row, config);
    harness.advance(Duration::from_millis(500));
    let mut host = Host {
        harness,
        requests,
        heard,
        workers,
        focus,
        calls,
        listed,
    };
    host.settle();
    host
}

/// A pane on `paths[at]` with the whole list to walk.
pub fn walking(paths: &[PathBuf], at: usize, viewport: Viewport) -> Host {
    hosted(
        vec![(paths[at].clone(), Some(sequence_of(paths, at)))],
        viewport,
    )
}

/// A text file of `lines` lines, each `<word> <number>`.
pub fn text_file(dir: &Path, name: &str, word: &str, lines: u32) -> PathBuf {
    let body: String = (1..=lines).map(|n| format!("{word} {n}\n")).collect();
    let path = dir.join(name);
    std::fs::write(&path, body).unwrap();
    std::fs::canonicalize(path).unwrap()
}

/// A copy of one of the image crate's fixtures.
pub fn image_file(dir: &Path, name: &str, fixture: &str) -> PathBuf {
    let from = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../anyview-image/tests/fixtures")
        .join(fixture);
    let path = dir.join(name);
    std::fs::copy(from, &path).unwrap();
    std::fs::canonicalize(path).unwrap()
}
