//! Assembling the program and running its event loop.

use super::relay::{
    Arrival, Doorway, WelcomeWhen, hand_on, listen, open_each, open_windows, relay, wants_of,
};
use super::role::{Role, claim_role};
use crate::cli::{CliError, Invocation, USAGE, parse};
use crate::crash;
use crate::host::{
    Appearances, CachedPictures, HelperHost, Hosting, ImageHost, Media, PATH_SETTLE, PathWatch,
    PlatformDesktop, PluginRegistry, PosterPictures, SETTLE, Services, Store, Watcher,
};
use crate::media::{MediaHub, MediaPlugins, NowPlaying, PlayerHost};
use crate::runtime::PoolSize;
use crate::seam::{NoticeWaker, Workforce};
use crate::window::{Factory, WINDOW};
use anyview_core::FilePath;
use anyview_media::AudioDriver;
use anyview_platform::portable::{FreedesktopThumbnails, NoStacking};
use anyview_platform::{Env, Instance, PluginRunner, Rejected, Request, discover};
use anyview_store::{STORE_FOLDER, Versions};
use ds_blitz::{
    AppConfig, AppHandle, AppId, Decorations, LastWindowClosed, TokioSpawner, launch_idle,
};
use ds_settings::{AppName, ConfigRoot, SystemPrefsSource};
use futures_channel::mpsc::unbounded;
use std::ffi::{OsStr, OsString};
use std::num::NonZeroUsize;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::{Builder, Runtime};

/// How long the viewer stays running with no window after the last one closes (`viewer.warm_for`'s
/// default), so the next open finds the device, the fonts and the instance warm. A constant until
/// quire's settings have the key (FINDINGS).
pub const WARM_FOR: Duration = Duration::from_secs(10 * 60);

/// How long a launch with no file waits for the request the bus may be about to deliver before it
/// shows the welcome window.
const WELCOME_AFTER: Duration = Duration::from_millis(400);

/// The desktop application id the windows carry.
const APP_ID: &str = "org.quire.Anyview";

/// Run the viewer as `args` (the command line without the program name) in `cwd`, in `env`.
pub fn run(args: &[OsString], cwd: &FilePath, env: Env) -> ExitCode {
    match parse(args, cwd) {
        Ok(Invocation::Help) => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Ok(Invocation::Version) => {
            println!("anyview {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Ok(Invocation::Launch(request)) => {
            crash::install(
                crash::crash_dir(&env.dirs.state),
                env!("CARGO_PKG_VERSION"),
                || anyview_store::system_clock()().0,
            );
            launch_viewer(request, env)
        }
        Err(error) => usage_error(&error),
    }
}

fn usage_error(error: &CliError) -> ExitCode {
    eprintln!("anyview: {error}\n{USAGE}");
    ExitCode::from(2)
}

fn launch_viewer(request: Request, env: Env) -> ExitCode {
    let runtime = match platform_runtime() {
        Ok(runtime) => runtime,
        Err(error) => {
            log::error!("cannot start the platform's runtime: {error}");
            return ExitCode::FAILURE;
        }
    };
    let role = runtime.block_on(claim_role(&instance_of(&env), &request));
    let (arrivals, inbox) = unbounded();
    let mut doorway = None;
    match role {
        Role::Forwarded => return ExitCode::SUCCESS,
        Role::Primary(primary) => {
            let (requests, door) = listen(primary);
            runtime.spawn(relay(requests, arrivals.clone()));
            doorway = Some(door);
        }
        Role::Alone(error) => {
            log::warn!("running without single instance: {error}");
        }
    }
    let first = wants_of(request);
    let own = arrivals.clone();
    if first.is_empty() {
        // A launch with no file shows a window to choose one in. A process the bus started has no
        // file of its own either, and the call that started it arrives just after the name is
        // owned: the welcome window waits a moment for it.
        runtime.spawn(async move {
            tokio::time::sleep(WELCOME_AFTER).await;
            let _gone = own.unbounded_send(Arrival::Welcome(WelcomeWhen::IfNoWindow));
        });
    } else {
        runtime.spawn(async move { open_each(first, &own).await });
    }
    // The viewer's own sender is dropped here; the relay (if any) keeps the channel open.
    drop(arrivals);
    show(runtime, env, inbox, doorway)
}

/// The single-instance claim of this build: the session bus (with bus activation) on the Linux
/// desktop, the per-user socket everywhere else.
#[cfg(all(feature = "quire-desktop", target_os = "linux"))]
fn instance_of(env: &Env) -> impl Instance {
    anyview_platform::linux::DbusInstance::new(env.clone())
}

/// The single-instance claim of this build: the per-user socket.
#[cfg(not(all(feature = "quire-desktop", target_os = "linux")))]
fn instance_of(_env: &Env) -> impl Instance {
    anyview_platform::portable::LatchkeyInstance::new()
}

/// The tokio runtime the platform's async calls run on: one worker, owned here.
fn platform_runtime() -> std::io::Result<Runtime> {
    Builder::new_multi_thread()
        .worker_threads(1)
        .thread_name("anyview-platform")
        .enable_all()
        .build()
}

fn show(
    runtime: Runtime,
    env: Env,
    inbox: futures_channel::mpsc::UnboundedReceiver<Arrival>,
    doorway: Option<Doorway>,
) -> ExitCode {
    let waker = NoticeWaker::default();
    let cores = std::thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
    let workforce = match Workforce::start(PoolSize::from_cores(cores), waker.clone()) {
        Ok(workforce) => Arc::new(workforce),
        Err(error) => {
            log::error!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let reporting = Arc::clone(&workforce);
    runtime.spawn(async move {
        loop {
            waker.woken().await;
            for notice in reporting.notices() {
                log::error!("a job ended without a result: {notice:?}");
            }
        }
    });
    let app = AppHandle::new();
    let audio = audio_driver(env.audio_output.as_deref());
    let for_bus = env.clone();
    let found = discover(&env);
    report_rejected(&found.rejected);
    // The registry is read again when a tool a plugin runs is installed, so a file that needed it
    // opens with it without a restart.
    let registry = PluginRegistry::rereading(found.plugins, {
        let env = env.clone();
        move || {
            let found = discover(&env);
            report_rejected(&found.rejected);
            found.plugins
        }
    });
    let helpers = helper_host(&env, &registry, can_install(&runtime));
    let mut image_host = ImageHost::following(registry.clone(), PluginRunner::default());
    let mut media_plugins = MediaPlugins::following(registry, PluginRunner::default());
    if let Some(helpers) = &helpers {
        image_host = image_host.offering(Arc::clone(helpers));
        media_plugins = media_plugins.offering(Arc::clone(helpers));
    }
    let plugins = Arc::new(media_plugins);
    // A tool the person installs with a package manager of their own is noticed as it appears.
    let _tools = helpers
        .as_ref()
        .and_then(|helpers| watch_tools(&runtime, helpers, &env.tool_path));
    let hub = MediaHub::start(
        runtime.handle(),
        move || async move { NowPlaying::register(&for_bus).await },
        Some(app.clone()),
        audio,
        Arc::<MediaPlugins>::clone(&plugins),
    );
    let scratch = env.dirs.cache.join(STORE_FOLDER);
    // The cuts and frames of a viewer that was killed mid-export are removed once, as the program starts.
    let sweeping = scratch.clone();
    runtime.spawn_blocking(move || {
        for prefix in ["trim", "frame"] {
            anyview_store::sweep_scratch(&sweeping, prefix);
        }
    });
    let media = Media {
        hub: hub.clone(),
        exports: workforce.exports(),
        scratch,
    };
    let store = Store::new(
        &env.dirs.data.join(STORE_FOLDER),
        anyview_store::system_clock(),
    );
    let hosting = Arc::new(PlatformDesktop::platform(
        runtime.handle().clone(),
        &env,
        Services {
            store,
            media,
            versions: Versions::under_state(&env.dirs.state),
            helpers: helpers.clone(),
            images: image_host.clone(),
        },
    ));
    // Old kept versions go once, as the program starts, off the window's threads.
    let pruning = Arc::clone(&hosting);
    runtime.spawn_blocking(move || pruning.prune_versions());
    let watcher = Watcher::start(SETTLE)
        .inspect_err(|error| log::warn!("changed files will not reload: {error}"))
        .ok()
        .map(Arc::new);
    let appearances = runtime.block_on(Appearances::follow(
        ds_settings::Store::new(ConfigRoot::Scratch(env.dirs.config.clone()), AppName::QUIRE),
        SystemPrefsSource::Portal,
        Arc::new(TokioSpawner::on(runtime.handle().clone())),
    ));
    let factory = Factory::new(
        workforce.workers(),
        Arc::clone(&hosting) as Arc<dyn Hosting>,
        Arc::new(PosterPictures::new(
            CachedPictures(FreedesktopThumbnails::new(&env)),
            Arc::clone(&plugins),
        )),
        watcher,
        appearances,
        Arc::new(PlayerHost::new(hub.clone())),
        Arc::new(NoStacking),
    )
    .with_window_screen(env.window_screen.as_deref())
    .with_image_plugins(Arc::new(image_host));
    let factory = match helpers {
        Some(helpers) => factory.with_helpers(helpers),
        None => factory,
    };
    runtime.spawn(open_windows(inbox, app.clone(), factory, hub.clone()));
    // A logout or a `kill` ends the program by the normal way out, so the places are kept.
    runtime.spawn(quit_on_signal(app.clone()));
    // No window of its own: every one is opened through the handle, the first as any other, so
    // closing any of them leaves the rest and the last one leaves the process warm for
    // `WARM_FOR`.
    let config = AppConfig::new("anyview", WINDOW)
        .with_app_id(AppId(APP_ID.to_owned()))
        .with_decorations(Decorations::Client)
        .with_last_window(LastWindowClosed::StayFor(WARM_FOR))
        .with_handle(app)
        .with_runtime(runtime.handle().clone());
    // The loop ends with the program's last window; a host that cannot run one is reported like
    // any other failure to start, after the same tidying, and the exit code says so.
    let launched = launch_idle(config);
    // The door closes first, and what came through it too late for a window is started again in a
    // viewer of its own: a launch that was told "taken" still gets its window.
    if let Some(doorway) = doorway {
        hand_on(doorway.close());
    }
    // The places still waiting to be kept are written before the runtime that waits on them ends,
    // and a player with no window ends with the program.
    hosting.flush();
    hub.stop_background();
    drop(workforce);
    // The relay and the report task wait on channels that never close; end them with the process.
    runtime.shutdown_background();
    match launched {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            log::error!("cannot show a window: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Ask the app to quit when the session ends the program (SIGTERM, SIGHUP), so the loop ends and the
/// places still waiting are written, the door closed and the players stopped.
#[cfg(unix)]
async fn quit_on_signal(app: AppHandle) {
    use tokio::signal::unix::{SignalKind, signal};
    let (Ok(mut terminate), Ok(mut hang_up)) = (
        signal(SignalKind::terminate()),
        signal(SignalKind::hangup()),
    ) else {
        log::warn!("cannot listen for the signal that ends the session");
        return;
    };
    tokio::select! {
        _ = terminate.recv() => {}
        _ = hang_up.recv() => {}
    }
    log::info!("asked to end; quitting");
    let _gone = app.quit();
}

/// The platform has no such signals to hear.
#[cfg(not(unix))]
async fn quit_on_signal(_app: AppHandle) {}

/// Say which manifests were found and cannot be used.
fn report_rejected(rejected: &[Rejected]) {
    for rejected in rejected {
        log::warn!(
            "plugin {} is not usable: {}",
            rejected.file.display(),
            rejected.error
        );
    }
}

/// The tools the plugins run and what installs them, from the file the viewer ships
/// (`<data dir>/quire/helpers/anyview.toml`, the person's own data directory first). Without the
/// file nothing is offered and a missing tool stays a row that names it.
fn helper_host(env: &Env, registry: &PluginRegistry, installable: bool) -> Option<Arc<HelperHost>> {
    let dirs: Vec<std::path::PathBuf> = std::iter::once(env.dirs.data.clone())
        .chain(env.dirs.data_dirs.iter().cloned())
        .collect();
    match ds_helpers::Catalog::load("anyview", &dirs) {
        Ok(catalog) => Some(Arc::new(
            HelperHost::new(
                catalog,
                ds_helpers::Environment::system(),
                ds_helpers::Installer::PackageKit(ds_helpers::PackageKit::system()),
                registry.clone(),
            )
            .installing_where(installable),
        )),
        Err(error) => {
            log::warn!("a missing tool will not be offered for install: {error}");
            None
        }
    }
}

/// Whether the system can install a package for a missing tool: quire's `Helpers` capability, which
/// asks the system bus whether PackageKit answers. Asked once, as the program starts. Without
/// `quire-desktop` the probe has no bus to ask and says no, so no Install... is offered.
fn can_install(runtime: &Runtime) -> bool {
    runtime
        .block_on(ds_desktop::Desktop::probe())
        .here(ds_desktop::Capability::Helpers)
}

/// Follow the tools: the windows are told when one appears, and the folders programs are found in
/// are watched so a package installed in a terminal is noticed. `None` when they cannot be watched.
fn watch_tools(runtime: &Runtime, helpers: &Arc<HelperHost>, path: &OsStr) -> Option<PathWatch> {
    runtime.spawn(helpers.following());
    let again = Arc::clone(helpers);
    PathWatch::start(path, PATH_SETTLE, move || again.look_again())
        .inspect_err(|error| log::warn!("installed tools will not be noticed: {error}"))
        .ok()
}

/// The audio driver the person asked for, or the system's own choice. A name that is not a
/// driver is said and ignored: the sound should still play.
fn audio_driver(asked: Option<&str>) -> AudioDriver {
    match asked {
        None => AudioDriver::Auto,
        Some(text) => AudioDriver::from_name(text).unwrap_or_else(|| {
            log::warn!("{text:?} is not an audio driver; using the system's");
            AudioDriver::Auto
        }),
    }
}
