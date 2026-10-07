//! The viewer's first window under the harness with the binary's own wiring: the runtime's pool
//! for the work, the desktop over fakes for the requests, and a real file to open.
#![allow(dead_code, clippy::unwrap_used)]

use anyview::host::Appearances;
use anyview::host::{
    CachedPictures, Clock, Desktop, HelperHost, ImageHost, Media, SETTLE, Services, Store, Trash,
    TrashError, Watcher,
};
use anyview::media::{MediaHub, MediaPlugins, PlayerHost};
use anyview::runtime::PoolSize;
use anyview::seam::{NoticeWaker, Workforce};
use anyview::window::{Factory, Opening, Seed, seeded_root};
use anyview_core::FilePath;
use anyview_media::AudioDriver;
use anyview_platform::testing::{
    FakeApps, FakeLinks, FakeMediaHandle, FakeMediaSession, FakePicker, FakePrinter, FakeReveal,
    FakeShare, FakeStacking, FakeThumbnails, PluginSet, StackingSupport, plugins_with,
};
use anyview_platform::{PickOutcome, PluginRunner, PrintOutcome};
use anyview_store::Viewed;
use anyview_ui::Look;
use ds_harness::{
    Backend, Clock as HarnessClock, Driver, Harness, HarnessConfig, SizerAck, Viewport,
    WindowScreen,
};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

pub const VIEW: Viewport = Viewport {
    width: 900,
    height: 600,
    scale_percent: 100,
};

/// A trash that does nothing: no test touches the person's.
struct NoTrash;

impl Trash for NoTrash {
    fn trash(&self, _file: &FilePath) -> Result<(), TrashError> {
        Ok(())
    }
}

/// A fixture of another crate of the workspace.
pub fn fixture(crate_dir: &str, name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(crate_dir)
        .join("tests/fixtures")
        .join(name)
}

/// Everything a window needs, kept alive for the test.
pub struct Rig {
    pub harness: Harness,
    pub workforce: Arc<Workforce>,
    pub store: PathBuf,
    /// The desktop's now-playing entry: what the program published and the controls it can press.
    pub now_playing: FakeMediaHandle,
    /// The program's players.
    pub media: MediaHub,
    /// When `open` began: the stand-in for the process's start.
    pub started: Instant,
    /// How long the program's own wiring took (pool, runtime, desktop, the folder listing).
    pub wired: Duration,
    /// How long it took until the window's first frame was up.
    pub windowed: Duration,
    _runtime: Runtime,
}

impl std::fmt::Debug for Rig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Rig").finish_non_exhaustive()
    }
}

/// The first window opened on `file`, wired as the binary wires it, with no plugin installed.
pub fn open(file: &Path, scratch: &Path) -> Rig {
    open_with(file, scratch, Arc::new(MediaPlugins::default()))
}

/// [`open`], with the plugins a run of the binary would have discovered.
pub fn open_with(file: &Path, scratch: &Path, plugins: Arc<MediaPlugins>) -> Rig {
    open_wired(file, scratch, plugins, Wired::default())
}

/// What a window is wired to beyond the players: the pictures' plugins, the tools that can be
/// installed when one is missing, and how the harness's window answers a request for a size and
/// which screen it says it is on.
#[derive(Default)]
pub struct Wired {
    pub images: Option<ImageHost>,
    pub helpers: Option<Arc<HelperHost>>,
    pub ack: SizerAck,
    pub screen: WindowScreen,
}

/// [`open_with`], wired as `wired` says.
pub fn open_wired(file: &Path, scratch: &Path, plugins: Arc<MediaPlugins>, wired: Wired) -> Rig {
    let started = Instant::now();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap();
    let workforce = Arc::new(
        Workforce::start(
            PoolSize::exactly(NonZeroUsize::new(2).unwrap()),
            NoticeWaker::default(),
        )
        .unwrap(),
    );
    let store = scratch.join("store");
    let now: Clock = Arc::new(|| Viewed(1_700_000_000));
    let session = FakeMediaSession::new();
    let now_playing = session.handle();
    let hub = MediaHub::start(
        runtime.handle(),
        move || std::future::ready(session),
        None,
        AudioDriver::Null,
        plugins,
    );
    let media = Media {
        hub: hub.clone(),
        exports: workforce.exports(),
        scratch: scratch.join("cache"),
    };
    let desktop = Desktop::new(
        runtime.handle().clone(),
        FakeApps::default(),
        FakeReveal::default(),
        FakeShare::default(),
        FakePrinter::answering(PrintOutcome::Printed),
        NoTrash,
        FakePicker::answering(PickOutcome::Cancelled),
        FakeLinks::default(),
        Services {
            versions: anyview_store::Versions::under_state(&scratch.join("state")),
            store: Store::new(&store, now),
            media,
            helpers: wired.helpers.clone(),
        },
    );
    let mut factory = Factory::new(
        workforce.workers(),
        Arc::new(desktop),
        Arc::new(CachedPictures(FakeThumbnails::default())),
        Watcher::start(SETTLE).ok().map(Arc::new),
        Appearances::fixed(Look::default()),
        Arc::new(PlayerHost::new(hub.clone())),
        Arc::new(FakeStacking::with(StackingSupport::Unsupported)),
    );
    if let Some(images) = wired.images {
        factory = factory.with_image_plugins(Arc::new(images));
    }
    if let Some(helpers) = wired.helpers {
        factory = factory.with_helpers(helpers);
    }
    let opening = Opening::around(FilePath::new(file).unwrap());
    let wiring_took = started.elapsed();
    let config = HarnessConfig::new(VIEW)
        .with_clock(HarnessClock::Virtual)
        .with_backend(Backend::Hybrid)
        .with_sizer_ack(wired.ack)
        .with_window_screen(wired.screen)
        .with_context(Seed {
            factory,
            opening,
            presentation: anyview_ui::Presentation::Window,
        });
    let harness = Harness::new(seeded_root, config);
    let windowed = started.elapsed();
    Rig {
        harness,
        workforce,
        store,
        now_playing,
        media: hub,
        started,
        wired: wiring_took,
        windowed,
        _runtime: runtime,
    }
}

/// Advance the window until `done` holds, letting the workers run in real time between steps.
/// Panics with the time it waited if `done` never holds.
pub fn until(
    harness: &mut Harness,
    what: &str,
    mut done: impl FnMut(&mut Harness) -> bool,
) -> Duration {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(20) {
        if done(harness) {
            return started.elapsed();
        }
        harness.advance(Duration::from_millis(5));
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("{what} did not happen within 20 s:\n{}", harness.html());
}

/// A fixture of the media crate, as an absolute path.
pub fn media_fixture(name: &str) -> FilePath {
    FilePath::new(std::fs::canonicalize(fixture("anyview-media", name)).unwrap()).unwrap()
}

/// A window's GPU as far as a player is concerned, made on a device with no window, or `None`
/// with a note where the machine has no adapter.
pub fn gpu() -> Option<ds_blitz::Gpu> {
    // One opening at a time in this process: the Vulkan loader crashes on two at once.
    let _one_at_a_time = anyview_media::GPU_OPENING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    for fallback in [false, true] {
        let Ok(adapter) =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: fallback,
            }))
        else {
            continue;
        };
        if let Ok((device, queue)) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("test-window"),
                ..Default::default()
            }))
        {
            return Some(ds_blitz::seam::attached_gpu(
                instance, adapter, device, queue,
            ));
        }
    }
    eprintln!("SKIPPED: no graphics adapter");
    None
}

/// Wait, in real time, until `done` holds; panics saying what it waited for.
pub fn eventually(what: &str, mut done: impl FnMut() -> bool) {
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(20) {
        if done() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("{what} did not happen within 20 s");
}

/// The programs that play, when this machine has them: the stock `mpv` (`MPV_WGPU_MPV`, else the
/// first on the search path) and the C plugin built from mpv-wgpu (`MPV_WGPU_CPLUGIN`).
pub fn mpv_programs() -> Option<(PathBuf, PathBuf)> {
    let cplugin = std::env::var_os("MPV_WGPU_CPLUGIN").map(PathBuf::from);
    let mpv = std::env::var_os("MPV_WGPU_MPV")
        .map(PathBuf::from)
        .or_else(|| on_path("mpv"));
    match (mpv, cplugin) {
        (Some(mpv), Some(cplugin)) if cplugin.is_file() => Some((mpv, cplugin)),
        (None, _) => {
            eprintln!("SKIPPED: no mpv (set MPV_WGPU_MPV or put mpv on the search path)");
            None
        }
        (Some(_), _) => {
            eprintln!("SKIPPED: no mpv-wgpu C plugin (set MPV_WGPU_CPLUGIN)");
            None
        }
    }
}

/// The program `name` on the search path, when there is one.
pub fn on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

/// The FFmpeg plugin built from this workspace, beside the test binaries' folder, when this
/// machine has FFmpeg for it to run (`cargo test --workspace` builds it; `cargo build -p
/// anyview-ffmpeg` does otherwise).
pub fn ffmpeg_program() -> Option<PathBuf> {
    if on_path("ffmpeg").is_none() || on_path("ffprobe").is_none() {
        eprintln!("SKIPPED: no ffmpeg and ffprobe on the search path");
        return None;
    }
    let tests = std::env::current_exe().ok()?;
    let built = tests.parent()?.parent()?.join("anyview-ffmpeg");
    if built.is_file() {
        Some(built)
    } else {
        eprintln!("SKIPPED: the FFmpeg plugin is not built (cargo build -p anyview-ffmpeg)");
        None
    }
}

/// Plugins for a test: whichever of playing and FFmpeg this machine has, as asked.
pub fn plugins(play: bool, ffmpeg: bool) -> Option<Arc<MediaPlugins>> {
    plugins_with_args(play, ffmpeg, &[])
}

/// [`plugins`], with `args` added to the FFmpeg plugin's command line.
pub fn plugins_with_args(play: bool, ffmpeg: bool, args: &[String]) -> Option<Arc<MediaPlugins>> {
    let programs = if play { Some(mpv_programs()?) } else { None };
    let program = if ffmpeg {
        Some(ffmpeg_program()?)
    } else {
        None
    };
    let set = PluginSet {
        play: programs
            .as_ref()
            .map(|(mpv, cplugin)| (mpv.as_path(), cplugin.as_path())),
        ffmpeg: program.as_deref(),
        ffmpeg_args: args,
    };
    Some(Arc::new(MediaPlugins::new(
        plugins_with(set),
        PluginRunner::default(),
    )))
}

/// `file` as the viewer's probe makes it: its type from its first bytes, as a recording needs no
/// look inside.
pub fn probed(file: &FilePath) -> anyview_ui::Probed {
    let bytes = std::fs::read(file.as_path()).unwrap();
    let name = anyview_core::FileName::new(file.file_name().unwrap().as_str()).unwrap();
    let head = anyview_core::FileHead::new(&bytes[..bytes.len().min(4096)]);
    let anyview_core::SniffStep::Done(sniffed) = anyview_core::sniff(&head, &name) else {
        panic!("a recording needs no look inside");
    };
    anyview_ui::Probed {
        source: anyview_core::Source::new(
            file.clone(),
            anyview_core::FileStamp {
                len: anyview_core::ByteLen(bytes.len() as u64),
                modified: anyview_core::ModTime(1),
            },
        ),
        family: anyview_ui::family_of(sniffed.kind()),
        sniffed,
        resume: anyview_core::Resume::Nothing,
        access: anyview_ui::FileAccess::Writable,
    }
}

/// `name` of the media crate's fixtures copied into `dir`, so what an export writes beside it
/// lands in the scratch folder.
pub fn copy_into(dir: &Path, name: &str) -> FilePath {
    let to = dir.join(name);
    std::fs::copy(media_fixture(name).as_path(), &to).unwrap();
    FilePath::new(std::fs::canonicalize(to).unwrap()).unwrap()
}

/// The desktop over fakes, with `hub`'s players and `workforce`'s pool for the media exports.
pub fn desktop(
    runtime: &Runtime,
    hub: &MediaHub,
    workforce: &Workforce,
    scratch: &Path,
) -> Arc<dyn anyview::host::Hosting> {
    let now: Clock = Arc::new(|| Viewed(1_700_000_000));
    Arc::new(Desktop::new(
        runtime.handle().clone(),
        FakeApps::default(),
        FakeReveal::default(),
        FakeShare::default(),
        FakePrinter::answering(PrintOutcome::Printed),
        NoTrash,
        FakePicker::answering(PickOutcome::Cancelled),
        FakeLinks::default(),
        Services {
            versions: anyview_store::Versions::under_state(&scratch.join("state")),
            store: Store::new(&scratch.join("store"), now),
            media: Media {
                hub: hub.clone(),
                exports: workforce.exports(),
                scratch: scratch.join("cache"),
            },
            helpers: None,
        },
    ))
}
