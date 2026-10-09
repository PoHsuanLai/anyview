//! A viewer window under the harness: workers that run each job where it is submitted, a scratch
//! folder of real files for the arrow keys to walk, and the requests the window made of its host.
#![allow(dead_code, unused_imports)]

mod player;

use anyview_core::{FilePath, FileStamp, NonEmpty, Resume, Sequence, SequenceOrigin};
use anyview_image::Rgba8;
pub use player::{Answer, FakeLine, FakePlayer};

use anyview_ui::{
    Edge, FileAccess, FileLocks, FirstFrameSource, HelperSource, HostRequest, ImagePlugins, Launch,
    LookFeed, MediaHost, PlatformAbilities, Presentation, ResumeSource, VersionRow, VersionSource,
    ViewerApp, Work, WorkKind, WorkLane, Workers,
};
use dioxus::prelude::*;
use ds::prelude::Appearance;
use ds::prelude::{Point, Px, ShortcutKey};
use ds::window::host::{HostWindow, use_window_host_provider};
use ds::window::vocab::{ResizeEdge, Support, TileError, WindowState, WindowTile, Zoom};
use ds_harness::{Backend, Clock, Driver, Harness, HarnessConfig, Viewport};
use ds_harness::{Input, Query};
use image::RgbaImage;
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The window's size in the tests.
pub const VIEW: Viewport = Viewport {
    width: 900,
    height: 600,
    scale_percent: 100,
};

/// Runs each job on the thread that submitted it, so a result is in the mailbox by the time the
/// harness looks.
#[derive(Debug)]
pub struct Inline;

impl Workers for Inline {
    fn submit(&self, work: Work) {
        work.run();
    }
}

/// The requests the window made of its host.
pub type Requests = Arc<Mutex<Vec<HostRequest>>>;

/// The fixture folders of the back-end crates.
pub fn fixture(crate_dir: &str, name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(crate_dir)
        .join("tests/fixtures")
        .join(name)
}

/// A scratch folder holding copies of `files` (crate folder, fixture name, name to give it), and
/// the paths in the order given.
pub fn folder(files: &[(&str, &str, &str)]) -> (tempfile::TempDir, Vec<PathBuf>) {
    let dir = tempfile::tempdir().unwrap();
    let paths = files
        .iter()
        .map(|(crate_dir, name, called)| {
            let to = dir.path().join(called);
            std::fs::copy(fixture(crate_dir, name), &to).unwrap();
            std::fs::canonicalize(to).unwrap()
        })
        .collect();
    (dir, paths)
}

/// Work held back until a test lets it go, and a log of everything the window submitted. With
/// nothing held it runs each job where it is submitted, as the inline workers do.
#[derive(Debug, Default)]
pub struct Gate {
    state: Mutex<GateState>,
}

#[derive(Debug, Default)]
struct GateState {
    holding: Vec<WorkKind>,
    held: Vec<Work>,
    log: Vec<(WorkKind, WorkLane)>,
}

impl Gate {
    /// A gate that holds the work of these `kinds` until it is released.
    pub fn holding(kinds: &[WorkKind]) -> Arc<Gate> {
        let gate = Gate::default();
        gate.state.lock().unwrap().holding = kinds.to_vec();
        Arc::new(gate)
    }

    /// What was submitted, in order, since the log was last taken.
    pub fn take_log(&self) -> Vec<(WorkKind, WorkLane)> {
        std::mem::take(&mut self.state.lock().unwrap().log)
    }

    /// How many pieces of work of `kind` are held.
    pub fn held(&self, kind: WorkKind) -> usize {
        let state = self.state.lock().unwrap();
        state.held.iter().filter(|work| work.kind() == kind).count()
    }

    /// Stop holding work of `kind` from now on, and run what is held of it, oldest first.
    pub fn release(&self, kind: WorkKind) {
        let ready: Vec<Work> = {
            let mut state = self.state.lock().unwrap();
            state.holding.retain(|held| *held != kind);
            let (ready, rest) = std::mem::take(&mut state.held)
                .into_iter()
                .partition(|work| work.kind() == kind);
            state.held = rest;
            ready
        };
        for work in ready {
            work.run();
        }
    }

    /// Run the oldest held work of `kind` and go on holding the rest.
    pub fn release_one(&self, kind: WorkKind) {
        let work = {
            let mut state = self.state.lock().unwrap();
            let at = state.held.iter().position(|work| work.kind() == kind);
            at.map(|at| state.held.remove(at))
        };
        if let Some(work) = work {
            work.run();
        }
    }
}

impl Workers for Gate {
    fn submit(&self, work: Work) {
        let hold = {
            let mut state = self.state.lock().unwrap();
            state.log.push((work.kind(), work.lane()));
            state.holding.contains(&work.kind())
        };
        if hold {
            self.state.lock().unwrap().held.push(work);
        } else {
            work.run();
        }
    }
}

/// Where each file was left, as a host's store keeps it: it hears the window's requests and
/// answers the window's reads, the way the program's host does.
#[derive(Debug, Default)]
pub struct Memory {
    state: Mutex<MemoryState>,
}

#[derive(Debug, Default)]
struct MemoryState {
    showing: Option<FilePath>,
    left: Vec<(FilePath, Resume)>,
}

impl Memory {
    /// A store that already remembers `resume` for `path`.
    pub fn with(path: &Path, resume: Resume) -> Arc<Memory> {
        let memory = Memory::default();
        memory
            .state
            .lock()
            .unwrap()
            .left
            .push((FilePath::new(path).unwrap(), resume));
        Arc::new(memory)
    }

    /// What the store would answer for `path`.
    pub fn left_at(&self, path: &Path) -> Option<Resume> {
        let path = FilePath::new(path).unwrap();
        let state = self.state.lock().unwrap();
        state
            .left
            .iter()
            .find(|(file, _)| *file == path)
            .map(|(_, resume)| resume.clone())
    }

    fn hear(&self, request: &HostRequest) {
        let mut state = self.state.lock().unwrap();
        match request {
            HostRequest::Opened(probed) => state.showing = Some(probed.source.path().clone()),
            HostRequest::Remember(resume) => {
                if let Some(path) = state.showing.clone() {
                    state.left.retain(|(file, _)| *file != path);
                    state.left.push((path, resume.clone()));
                }
            }
            HostRequest::Run(_)
            | HostRequest::PickFile
            | HostRequest::CloseWindow
            | HostRequest::Export(_)
            | HostRequest::Trash
            | HostRequest::Rename(_)
            | HostRequest::Edit(_)
            | HostRequest::Rewind(_)
            | HostRequest::RevertTo(_)
            | HostRequest::SaveCopy(_)
            | HostRequest::Watch(_)
            | HostRequest::Unwatch
            | HostRequest::OpenUri(_)
            | HostRequest::OpenFiles(_)
            | HostRequest::Reveal(_)
            | HostRequest::Provide(_)
            | HostRequest::SizeWindow(_)
            | HostRequest::Present(_) => {}
        }
    }
}

impl ResumeSource for Memory {
    fn recall(&self, path: &FilePath, _stamp: FileStamp) -> Resume {
        let state = self.state.lock().unwrap();
        state
            .left
            .iter()
            .find(|(file, _)| file == path)
            .map_or(Resume::Nothing, |(_, resume)| resume.clone())
    }
}

/// Small pictures the host has, by file name: what a thumbnail cache would hold.
#[derive(Debug, Default)]
pub struct Pictures(pub Vec<(String, Rgba8)>);

impl FirstFrameSource for Pictures {
    fn picture(&self, source: &anyview_core::Source) -> Option<Rgba8> {
        let name = source.path().file_name()?;
        self.0
            .iter()
            .find(|(wanted, _)| wanted == name.as_str())
            .map(|(_, picture)| picture.clone())
    }
}

/// Versions the host keeps of every file, the same list for each.
#[derive(Debug, Default)]
pub struct Versions(pub Vec<VersionRow>);

impl VersionSource for Versions {
    fn list(&self, _path: &FilePath) -> Vec<VersionRow> {
        self.0.clone()
    }
}

/// Files the host reports as refusing a save in place, by path.
#[derive(Debug, Default)]
pub struct Locks(pub Vec<PathBuf>);

impl FileLocks for Locks {
    fn access(&self, path: &FilePath) -> FileAccess {
        if self.0.iter().any(|locked| locked == path.as_path()) {
            FileAccess::ReadOnly
        } else {
            FileAccess::Writable
        }
    }
}

/// What a window under test is wired to besides its files.
#[derive(Default)]
pub struct Wiring {
    /// The pool; the inline workers when none.
    pub workers: Option<Arc<dyn Workers>>,
    /// Where files were left; nothing when none.
    pub memory: Option<Arc<Memory>>,
    /// The host's small pictures; none when none.
    pub pictures: Option<Arc<Pictures>>,
    /// The versions the host keeps; none when none.
    pub versions: Option<Arc<Versions>>,
    /// The host's players; none when none.
    pub player: Option<Arc<FakePlayer>>,
    /// The files that refuse a save in place; every file takes one when none.
    pub locks: Option<Arc<Locks>>,
    /// The plugins that decode what the viewer cannot; none when none.
    pub image_plugins: Option<Arc<dyn ImagePlugins>>,
    /// What the install sheet says of each tool; nothing is offered when none.
    pub helpers: Option<Arc<dyn HelperSource>>,
    /// What the platform can do; every ability when none.
    pub platform: Option<PlatformAbilities>,
    /// How the window is on screen.
    pub presentation: Presentation,
    /// The desktop's look as it changes; the launch look for good when none.
    pub feed: Option<LookFeed>,
    /// The window's size; `VIEW` when none.
    pub viewport: Option<Viewport>,
    /// Give the viewer a window host that counts the moves it is asked for ([`window_moves`]).
    pub record_moves: bool,
}

thread_local! {
    static MOVES: Cell<usize> = const { Cell::new(0) };
}

/// A window host that only counts the interactive moves the viewer asks for.
struct CountsMoves;

impl HostWindow for CountsMoves {
    fn begin_move(&self) {
        MOVES.with(|moves| moves.set(moves.get() + 1));
    }
    fn begin_resize(&self, _edge: ResizeEdge) {}
    fn zoom(&self, _zoom: Zoom) {}
    fn minimize(&self) {}
    fn close(&self) {}
    fn tile(&self, _tile: WindowTile) -> Result<(), TileError> {
        Err(TileError::Unsupported)
    }
    fn supports(&self, _tile: WindowTile) -> Support {
        Support::No
    }
    fn state(&self) -> WindowState {
        WindowState::default()
    }
}

/// The viewer under a window host that counts moves.
#[allow(non_snake_case)]
fn MovesRecorded() -> Element {
    use_window_host_provider(|| Rc::new(CountsMoves));
    rsx! { ViewerApp {} }
}

/// How many window moves the viewer has asked for on this thread since [`wired`] built it
/// with `record_moves`.
pub fn window_moves() -> usize {
    MOVES.with(Cell::get)
}

/// A viewer window opened on `paths[at]` with the whole list to walk.
pub fn window(paths: &[PathBuf], at: usize, appearance: Appearance) -> (Harness, Requests) {
    let (harness, requests, _) = wired(paths, at, appearance, Wiring::default());
    (harness, requests)
}

/// A viewer window opened on `paths[at]` with the whole list to walk, wired as `wiring` says. The
/// edge is returned so the test can tell the window that a file changed, as the host would.
pub fn wired(
    paths: &[PathBuf],
    at: usize,
    appearance: Appearance,
    wiring: Wiring,
) -> (Harness, Requests, Edge) {
    let entries: Vec<FilePath> = paths
        .iter()
        .map(|path| FilePath::new(path).unwrap())
        .collect();
    let current = entries[at].clone();
    let sequence = Sequence::starting_at(
        NonEmpty::from_vec(entries).unwrap(),
        &current,
        SequenceOrigin::Selection,
    )
    .unwrap();
    let requests: Requests = Arc::default();
    let seen = Arc::clone(&requests);
    let memory = wiring.memory.clone();
    let workers: Arc<dyn Workers> = wiring.workers.unwrap_or_else(|| Arc::new(Inline));
    let mut edge = Edge::new(workers, move |request| {
        if let Some(memory) = &memory {
            memory.hear(&request);
        }
        seen.lock().unwrap().push(request);
    });
    if let Some(memory) = wiring.memory {
        edge = edge.with_resume_source(memory);
    }
    if let Some(versions) = wiring.versions {
        edge = edge.with_version_source(versions);
    }
    if let Some(pictures) = wiring.pictures {
        edge = edge.with_first_frames(pictures);
    }
    if let Some(locks) = wiring.locks {
        edge = edge.with_locks(locks);
    }
    if let Some(plugins) = wiring.image_plugins {
        edge = edge.with_image_plugins(plugins);
    }
    if let Some(helpers) = wiring.helpers {
        edge = edge.with_helpers(helpers);
    }
    if let Some(platform) = wiring.platform {
        edge = edge.with_platform(platform);
    }
    if let Some(player) = wiring.player {
        edge = edge.with_media(player as Arc<dyn MediaHost>);
    }
    let launch = Launch {
        file: current,
        sequence: Some(sequence),
        look: appearance.into(),
        presentation: wiring.presentation,
    };
    let mut config = HarnessConfig::new(wiring.viewport.unwrap_or(VIEW))
        .with_clock(Clock::Virtual)
        .with_backend(Backend::Hybrid)
        .with_context(edge.clone())
        .with_context(launch);
    if let Some(feed) = wiring.feed {
        config = config.with_context(feed);
    }
    MOVES.with(|moves| moves.set(0));
    let app: fn() -> Element = if wiring.record_moves {
        MovesRecorded
    } else {
        ViewerApp
    };
    let mut harness = Harness::new(app, config);
    harness.advance(Duration::from_millis(500));
    (harness, requests, edge)
}

/// Where a picture is saved for a person to look at.
pub fn shot(name: &str) -> Option<PathBuf> {
    std::env::var_os("ANYVIEW_SHOTS").map(|dir| Path::new(&dir).join(name))
}

pub fn middle() -> Point {
    Point {
        x: Px(VIEW.width as f32 / 2.0),
        y: Px(VIEW.height as f32 / 2.0),
    }
}

/// A text file of `lines` lines, each `<word> <number>`, in a folder of its own.
pub fn text_file(dir: &Path, name: &str, word: &str, lines: u32) -> PathBuf {
    let body: String = (1..=lines).map(|n| format!("{word} {n}\n")).collect();
    let path = dir.join(name);
    std::fs::write(&path, body).unwrap();
    std::fs::canonicalize(path).unwrap()
}

/// A copy of one of the image crate's fixtures.
pub fn image_file(dir: &Path, name: &str, fixture_name: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::copy(fixture("anyview-image", fixture_name), &path).unwrap();
    std::fs::canonicalize(path).unwrap()
}

pub fn settle(harness: &mut Harness) {
    harness.advance(Duration::from_millis(300));
}

pub fn press(harness: &mut Harness, key: ShortcutKey) {
    harness.send(Input::key(key));
    settle(harness);
}

pub fn title(harness: &Harness) -> Option<String> {
    harness.text_of(".ds-titlebar-name")
}

/// The number of the first line on screen.
pub fn first_line(harness: &Harness) -> Option<u32> {
    harness.text_of(".viewer-lineno")?.trim().parse().ok()
}

pub fn rgb(image: &RgbaImage, dx: f32, dy: f32) -> [u8; 3] {
    let at = (
        (middle().x.0 + dx).round() as u32,
        (middle().y.0 + dy).round() as u32,
    );
    let pixel = image.get_pixel(at.0, at.1);
    [pixel[0], pixel[1], pixel[2]]
}

pub fn is(colour: [u8; 3], want: [u8; 3]) -> bool {
    colour
        .iter()
        .zip(want)
        .all(|(got, want)| (i32::from(*got) - i32::from(want)).abs() < 40)
}

pub const RED: [u8; 3] = [255, 0, 0];
pub const GREEN: [u8; 3] = [0, 255, 0];
pub const MAGENTA: [u8; 3] = [255, 0, 255];
