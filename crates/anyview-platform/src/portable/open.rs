//! Opening an address, and showing a file in its folder, with the program the platform ships for
//! it: `xdg-open` on Linux and the other Unixes, `open` on macOS, `explorer` on Windows. Each is
//! started through [`Env`]'s [`Spawn`](crate::Spawn) as a program and its arguments, never as a
//! shell string, so a path or an address with spaces or quotes in it is one argument. Inside a
//! sandbox `xdg-open` itself goes through the OpenURI portal.

use crate::env::Env;
use crate::error::PlatformError;
use crate::link::OpenLink;
use crate::reveal::Reveal;
use crate::spawn::Argv;
use anyview_core::FilePath;

/// Whose opener to ask. A value rather than a `cfg!`, so every platform's command line is
/// checked by a test on any machine; the real one is chosen once, in [`Os::here`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Os {
    Unix,
    Mac,
    Windows,
}

impl Os {
    const fn here() -> Os {
        if cfg!(target_os = "macos") {
            Os::Mac
        } else if cfg!(target_os = "windows") {
            Os::Windows
        } else {
            Os::Unix
        }
    }
}

/// The command that opens `target` (an address or a path).
fn open_argv(os: Os, target: &str) -> Argv {
    match os {
        // `--` so a target that begins with a dash is never read as an option.
        Os::Unix => Argv::new("xdg-open", vec!["--".to_owned(), target.to_owned()]),
        Os::Mac => Argv::new("open", vec!["--".to_owned(), target.to_owned()]),
        Os::Windows => Argv::new("explorer", vec![target.to_owned()]),
    }
}

/// The command that shows `file` in its folder. A bare Unix has no way to select a file in the
/// file manager, so it opens the folder.
fn reveal_argv(os: Os, file: &FilePath) -> Argv {
    let path = file.as_path().to_string_lossy().into_owned();
    match os {
        Os::Unix => {
            let folder = file.parent().map_or_else(
                || path.clone(),
                |dir| dir.as_path().to_string_lossy().into_owned(),
            );
            open_argv(os, &folder)
        }
        Os::Mac => Argv::new("open", vec!["-R".to_owned(), "--".to_owned(), path]),
        Os::Windows => Argv::new("explorer", vec![format!("/select,{path}")]),
    }
}

/// [`OpenLink`] through the platform's opener.
#[derive(Debug, Clone)]
pub struct SystemOpen {
    env: Env,
}

impl SystemOpen {
    /// Links opened by starting programs through `env`.
    pub fn new(env: Env) -> Self {
        SystemOpen { env }
    }
}

impl OpenLink for SystemOpen {
    fn open(&self, uri: &str) -> Result<(), PlatformError> {
        self.env.spawn.spawn(&open_argv(Os::here(), uri))
    }
}

/// [`Reveal`] through the platform's opener: the file selected in the file manager where the
/// platform's opener can (macOS, Windows), else its folder.
#[derive(Debug, Clone)]
pub struct SystemReveal {
    env: Env,
}

impl SystemReveal {
    /// A reveal that starts programs through `env`.
    pub fn new(env: Env) -> Self {
        SystemReveal { env }
    }
}

impl Reveal for SystemReveal {
    async fn reveal(&self, file: &FilePath) -> Result<(), PlatformError> {
        self.env.spawn.spawn(&reveal_argv(Os::here(), file))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::testing::RecordingSpawn;
    use std::sync::Arc;

    fn recording() -> (Env, RecordingSpawn) {
        let spawn = RecordingSpawn::default();
        let mut env = Env::isolated(std::path::Path::new("/nonexistent"));
        env.spawn = Arc::new(spawn.clone());
        (env, spawn)
    }

    #[test]
    fn an_address_is_handed_to_the_opener_after_a_double_dash() {
        let (env, spawn) = recording();
        SystemOpen::new(env)
            .open("https://example.org/a b")
            .unwrap();
        let started = spawn.started();
        assert_eq!(started.len(), 1);
        let want = open_argv(Os::here(), "https://example.org/a b");
        assert_eq!(started[0], want);
        if Os::here() == Os::Unix {
            assert_eq!(started[0].program(), "xdg-open");
            assert_eq!(started[0].args(), ["--", "https://example.org/a b"]);
        }
    }

    #[test]
    fn each_platform_opens_and_reveals_with_its_own_program() {
        let file = FilePath::new("/pics/a b/-c.png").unwrap();
        let cases = [
            (Os::Unix, "xdg-open", vec!["--", "/pics/a b"]),
            (Os::Mac, "open", vec!["-R", "--", "/pics/a b/-c.png"]),
            (Os::Windows, "explorer", vec!["/select,/pics/a b/-c.png"]),
        ];
        for (os, program, args) in cases {
            let argv = reveal_argv(os, &file);
            assert_eq!(argv.program(), program, "{os:?}");
            assert_eq!(argv.args(), args, "{os:?}");
        }
        assert_eq!(open_argv(Os::Mac, "-x").args(), ["--", "-x"]);
        assert_eq!(open_argv(Os::Windows, "https://a").program(), "explorer");
    }

    #[tokio::test]
    async fn revealing_starts_the_opener_as_arguments_not_a_shell_line() {
        let (env, spawn) = recording();
        let file = FilePath::new("/pics/it's \"a\" $(file).png").unwrap();
        SystemReveal::new(env).reveal(&file).await.unwrap();
        let started = spawn.started();
        assert_eq!(started, [reveal_argv(Os::here(), &file)]);
        assert!(
            started[0]
                .args()
                .iter()
                .all(|arg| !arg.contains(' ') || arg.contains('/'))
        );
    }
}
