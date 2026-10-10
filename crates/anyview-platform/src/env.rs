//! Everything the edge reaches outside the process, chosen once by the binary: the person's
//! directories, the session bus and the way a program is started. No implementation reads an
//! environment variable, a directory or a bus address itself (CONVENTIONS section 6); this file
//! is the one place the process is asked.

use crate::spawn::{ProcessSpawn, RefuseSpawn, Spawn};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Where the person's files, caches and application entries live.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Dirs {
    /// `$HOME`.
    pub home: PathBuf,
    /// `$XDG_CONFIG_HOME`, else `~/.config`.
    pub config: PathBuf,
    /// `$XDG_DATA_HOME`, else `~/.local/share`.
    pub data: PathBuf,
    /// `$XDG_CACHE_HOME`, else `~/.cache`.
    pub cache: PathBuf,
    /// `$XDG_STATE_HOME`, else `~/.local/state`: what must outlive a cache clean but is not a
    /// setting, such as the originals kept before a save in place.
    pub state: PathBuf,
    /// `$XDG_CONFIG_DIRS`, most important first.
    pub config_dirs: Vec<PathBuf>,
    /// `$XDG_DATA_DIRS`, most important first.
    pub data_dirs: Vec<PathBuf>,
}

impl Dirs {
    /// This process's, as the `dirs` crate and the XDG variables name them. Without a home at
    /// all everything hangs off `/`, where writes fail and are reported.
    #[must_use]
    pub fn from_process() -> Dirs {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        Dirs {
            config: dirs::config_dir().unwrap_or_else(|| home.join(".config")),
            data: dirs::data_dir().unwrap_or_else(|| home.join(".local/share")),
            cache: dirs::cache_dir().unwrap_or_else(|| home.join(".cache")),
            state: dirs::state_dir().unwrap_or_else(|| home.join(".local/state")),
            config_dirs: search_path("XDG_CONFIG_DIRS", "/etc/xdg"),
            data_dirs: search_path("XDG_DATA_DIRS", "/usr/local/share:/usr/share"),
            home,
        }
    }

    /// Every directory inside `root` (a test's scratch folder): `root/home`, `root/config`,
    /// `root/data`, `root/cache`, `root/state`, and one system directory each, `root/etc-xdg` and
    /// `root/share`.
    #[must_use]
    pub fn under(root: &Path) -> Dirs {
        Dirs {
            home: root.join("home"),
            config: root.join("config"),
            data: root.join("data"),
            cache: root.join("cache"),
            state: root.join("state"),
            config_dirs: vec![root.join("etc-xdg")],
            data_dirs: vec![root.join("share")],
        }
    }
}

/// A colon-separated search path variable, or `default` when it is unset or empty.
fn search_path(variable: &str, default: &str) -> Vec<PathBuf> {
    let text = std::env::var(variable)
        .ok()
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| default.to_owned());
    text.split(':')
        .filter(|part| !part.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// How the session bus is reached.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum BusRoute {
    /// The process's own: `DBUS_SESSION_BUS_ADDRESS`, else the standard socket.
    #[default]
    Usual,
    /// A bus by address: a private `dbus-daemon` (tests, dev scripts).
    Address(String),
    /// No bus at all: whatever needs one reports [`PlatformError::NoBus`].
    Absent,
}

/// The seams every implementation is built with: start from [`Env::from_process`] or
/// [`Env::isolated`] and change a seam with a `with_*` method.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Env {
    /// The person's directories.
    pub dirs: Dirs,
    /// The session bus.
    pub session: BusRoute,
    /// How a program is started.
    pub spawn: Arc<dyn Spawn>,
    /// The audio driver the person asked for (`ANYVIEW_AUDIO_OUTPUT`), as they wrote it: the
    /// player parses it, and without one it lets the system's sound server choose.
    pub audio_output: Option<String>,
    /// `PATH`: the folders programs are found in, as the system wrote it.
    pub tool_path: OsString,
    /// The screen to fit windows to in place of the desktop's (`ANYVIEW_WINDOW_SCREEN`), as they
    /// wrote it: `WIDTHxHEIGHT` logical pixels, parsed by the window. For tests.
    pub window_screen: Option<String>,
}

impl Env {
    /// The process's own directories, session bus and child processes.
    #[must_use]
    pub fn from_process() -> Env {
        Env {
            dirs: Dirs::from_process(),
            session: BusRoute::Usual,
            spawn: Arc::new(ProcessSpawn),
            audio_output: std::env::var("ANYVIEW_AUDIO_OUTPUT")
                .ok()
                .filter(|text| !text.is_empty()),
            tool_path: std::env::var_os("PATH").unwrap_or_default(),
            window_screen: std::env::var("ANYVIEW_WINDOW_SCREEN")
                .ok()
                .filter(|text| !text.is_empty()),
        }
    }

    /// Nothing outside `scratch`: every directory inside it, no bus, and no program ever
    /// started. For tests that must never reach the person's session or files.
    #[must_use]
    pub fn isolated(scratch: &Path) -> Env {
        Env {
            dirs: Dirs::under(scratch),
            session: BusRoute::Absent,
            spawn: Arc::new(RefuseSpawn),
            audio_output: None,
            tool_path: OsString::new(),
            window_screen: None,
        }
    }

    /// The same environment with the person's directories `dirs`.
    #[must_use]
    pub fn with_dirs(self, dirs: Dirs) -> Env {
        Env { dirs, ..self }
    }

    /// The same environment reaching the session bus by `session`.
    #[must_use]
    pub fn with_session(self, session: BusRoute) -> Env {
        Env { session, ..self }
    }

    /// The same environment starting programs with `spawn`.
    #[must_use]
    pub fn with_spawn(self, spawn: Arc<dyn Spawn>) -> Env {
        Env { spawn, ..self }
    }

    /// The same environment finding programs in the folders of `tool_path`.
    #[must_use]
    pub fn with_tool_path(self, tool_path: OsString) -> Env {
        Env { tool_path, ..self }
    }

    /// The same environment fitting windows to the screen `window_screen` names.
    #[must_use]
    pub fn with_window_screen(self, window_screen: Option<String>) -> Env {
        Env {
            window_screen,
            ..self
        }
    }

    /// The same environment with the audio driver the person asked for, as they wrote it.
    #[must_use]
    pub fn with_audio_output(self, audio_output: Option<String>) -> Env {
        Env {
            audio_output,
            ..self
        }
    }
}
