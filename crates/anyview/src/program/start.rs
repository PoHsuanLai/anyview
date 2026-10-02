//! Assembling the program and running its event loop.

use super::relay::{Arrival, open_each, open_windows, relay, wants_of};
use super::role::{Role, claim_role};
use crate::cli::{CliError, Invocation, USAGE, parse};
use crate::host::{CachedPictures, Clock, Hosting, LinuxDesktop, Media, SETTLE, Store, Watcher};
use crate::media::{MediaHub, NowPlaying, PlayerHost};
use crate::runtime::PoolSize;
use crate::seam::{NoticeWaker, Workforce};
use crate::window::Factory;
use anyview_core::FilePath;
use anyview_media::AudioDriver;
use anyview_platform::linux::{DbusInstance, FreedesktopThumbnails, NoStacking};
use anyview_platform::{Env, Request};
use anyview_store::Viewed;
use ds::prelude::Appearance;
use ds_blitz::{AppConfig, AppHandle, AppId, Decorations, LastWindowClosed, launch_idle};
use futures_channel::mpsc::unbounded;
use std::ffi::OsString;
use std::num::NonZeroUsize;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::runtime::{Builder, Runtime};

/// How long the viewer stays running with no window after the last one closes (`viewer.warm_for`'s
/// default), so the next open finds the device, the fonts and the instance warm. A constant until
/// quire's settings have the key (FINDINGS).
pub const WARM_FOR: Duration = Duration::from_secs(10 * 60);

/// The folder under the person's data directory that holds the history and the view memory. The
/// launcher reads the same one.
const STORE_FOLDER: &str = "anyview";

/// The desktop application id the windows carry.
const APP_ID: &str = "org.quire.Anyview";

/// Run the viewer as `args` (the command line without the program name) in `cwd`, in `env`.
pub fn run(args: &[OsString], cwd: &FilePath, env: Env) -> ExitCode {
    match parse(args, cwd) {
        Ok(Invocation::Help) => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Ok(Invocation::Launch(request)) => launch_viewer(request, env),
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
            eprintln!("anyview: cannot start the platform's runtime: {error}");
            return ExitCode::FAILURE;
        }
    };
    let role = runtime.block_on(claim_role(&DbusInstance::new(env.clone()), &request));
    let (arrivals, inbox) = unbounded();
    // A process the bus started has no file of its own: the call that started it arrives once
    // the name is owned. Without the bus nothing else will ever ask, so a launch with no file is
    // a usage error.
    let may_wait = matches!(role, Role::Primary(_));
    match role {
        Role::Forwarded => return ExitCode::SUCCESS,
        Role::Primary(primary) => {
            runtime.spawn(relay(primary, arrivals.clone()));
        }
        Role::Alone(error) => {
            eprintln!("anyview: running without single instance: {error}");
        }
    }
    let first = wants_of(request);
    if first.is_empty() && !may_wait {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let own = arrivals.clone();
    runtime.spawn(async move { open_each(first, &own).await });
    // The viewer's own sender is dropped here; the relay (if any) keeps the channel open.
    drop(arrivals);
    show(runtime, env, inbox)
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
) -> ExitCode {
    let waker = NoticeWaker::default();
    let cores = std::thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
    let workforce = match Workforce::start(PoolSize::from_cores(cores), waker.clone()) {
        Ok(workforce) => Arc::new(workforce),
        Err(error) => {
            eprintln!("anyview: {error}");
            return ExitCode::FAILURE;
        }
    };
    let reporting = Arc::clone(&workforce);
    runtime.spawn(async move {
        loop {
            waker.woken().await;
            for notice in reporting.notices() {
                eprintln!("anyview: a job ended without a result: {notice:?}");
            }
        }
    });
    let app = AppHandle::new();
    let audio = audio_driver(env.audio_output.as_deref());
    let for_bus = env.clone();
    let hub = MediaHub::start(
        runtime.handle(),
        move || async move { NowPlaying::register(&for_bus).await },
        Some(app.clone()),
        audio,
    );
    let media = Media {
        hub: hub.clone(),
        exports: workforce.exports(),
        scratch: env.dirs.cache.join(STORE_FOLDER),
    };
    let store = Store::new(&env.dirs.data.join(STORE_FOLDER), clock());
    let hosting = Arc::new(LinuxDesktop::linux(
        runtime.handle().clone(),
        &env,
        store,
        media,
    ));
    let watcher = Watcher::start(SETTLE)
        .inspect_err(|error| eprintln!("anyview: changed files will not reload: {error}"))
        .ok()
        .map(Arc::new);
    let factory = Factory::new(
        workforce.workers(),
        Arc::clone(&hosting) as Arc<dyn Hosting>,
        Arc::new(CachedPictures(FreedesktopThumbnails::new(&env))),
        watcher,
        Appearance::default(),
        Arc::new(PlayerHost::new(hub.clone())),
        Arc::new(NoStacking),
    );
    runtime.spawn(open_windows(inbox, app.clone(), factory, hub.clone()));
    // No window of its own: every one is opened through the handle, the first as any other, so
    // closing any of them leaves the rest and the last one leaves the process warm for
    // `WARM_FOR`.
    let config = AppConfig::new("anyview", 1000, 700)
        .with_app_id(AppId(APP_ID.to_owned()))
        .with_decorations(Decorations::Client)
        .with_last_window(LastWindowClosed::StayFor(WARM_FOR))
        .with_handle(app);
    launch_idle(config);
    // The places still waiting to be kept are written before the runtime that waits on them ends,
    // and a player with no window ends with the program.
    hosting.flush();
    hub.stop_background();
    drop(workforce);
    // The relay and the report task wait on channels that never close; end them with the process.
    runtime.shutdown_background();
    ExitCode::SUCCESS
}

/// The audio driver the person asked for, or the system's own choice. A name that is not a
/// driver is said and ignored: the sound should still play.
fn audio_driver(asked: Option<&str>) -> AudioDriver {
    match asked {
        None => AudioDriver::Auto,
        Some(text) => AudioDriver::from_name(text).unwrap_or_else(|| {
            eprintln!("anyview: {text:?} is not an audio driver; using the system's");
            AudioDriver::Auto
        }),
    }
}

/// The wall clock, in whole seconds since the epoch. The one place the program reads the time.
fn clock() -> Clock {
    Arc::new(|| {
        Viewed(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |since| since.as_secs()),
        )
    })
}
