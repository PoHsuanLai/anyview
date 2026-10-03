//! A scratch desktop for the plugin's tests: XDG directories under a temporary folder, the shipped
//! manifest template filled in for the plugin built for this run, the fixtures of the media crate,
//! and the checks that need FFmpeg skip with a message when it is absent.

#![allow(dead_code, clippy::unwrap_used)]

use anyview_core::FilePath;
use anyview_platform::{Env, discover};
use anyview_plugin::Installed;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

/// The plugin's executable, built for this test run.
pub const PLUGIN: &str = env!("CARGO_BIN_EXE_anyview-ffmpeg");

/// The manifest template the installer fills in.
const TEMPLATE: &str = include_str!("../../../../dist/plugins/anyview-ffmpeg.toml.in");

/// The program `name` on the search path, when there is one.
pub fn on_path(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|dir| dir.join(name))
            .find(|candidate| candidate.is_file())
    })
}

/// Whether ffmpeg and ffprobe are here; says so when they are not.
pub fn have_ffmpeg() -> bool {
    let have = on_path("ffmpeg").is_some() && on_path("ffprobe").is_some();
    if !have {
        eprintln!("skipped: no ffmpeg");
    }
    have
}

/// Ends the test early, saying why, when FFmpeg is not installed.
#[macro_export]
macro_rules! require_ffmpeg {
    () => {
        if !$crate::support::have_ffmpeg() {
            return;
        }
    };
}

/// A fixture of the media crate.
pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/anyview-media/tests/fixtures")
        .join(name)
}

pub fn file(path: &Path) -> FilePath {
    FilePath::new(path).unwrap()
}

pub struct Scratch {
    pub dir: TempDir,
    pub env: Env,
}

impl Scratch {
    pub fn new() -> Scratch {
        let dir = TempDir::new().unwrap();
        let env = Env::isolated(dir.path());
        Scratch { dir, env }
    }

    /// Installs the shipped manifest for the built plugin, with `args` on its command line.
    pub fn install(&self, args: &[String]) -> Installed {
        let folder = self.env.dirs.data.join("anyview/plugins");
        fs::create_dir_all(&folder).unwrap();
        let args: Vec<String> = args.iter().map(|arg| format!("{arg:?}")).collect();
        let text = TEMPLATE
            .replace("@PREFIX@/libexec/anyview/anyview-ffmpeg", PLUGIN)
            .replace("args = []", &format!("args = [{}]", args.join(", ")));
        fs::write(folder.join("ffmpeg.toml"), text).unwrap();
        let found = discover(&self.env);
        assert!(found.rejected.is_empty(), "{:?}", found.rejected);
        found
            .plugins
            .installed()
            .iter()
            .find(|plugin| plugin.manifest.id.as_str() == "ffmpeg")
            .cloned()
            .expect("the manifest is discovered")
    }

    /// A path in the scratch folder.
    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    /// A program that stands in for ffmpeg: it answers `-version` and `-encoders` (FLAC only), and
    /// for anything else runs `body` with the last argument, the file being written, in `$out`.
    pub fn fake_ffmpeg(&self, body: &str) -> PathBuf {
        let path = self.path("fake-ffmpeg");
        let script = format!(
            "#!/bin/sh\n\
             case \"$1\" in\n\
             -version) echo 'ffmpeg version 8.1 fake'; exit 0;;\n\
             -hide_banner) printf 'Encoders:\\n ------\\n A....D flac x\\n'; exit 0;;\n\
             esac\n\
             for out; do :; done\n\
             {body}\n"
        );
        fs::write(&path, script).unwrap();
        let mut perms = fs::metadata(&path).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
        fs::set_permissions(&path, perms).unwrap();
        path
    }
}

/// `--ffmpeg <path>` and the real ffprobe, as manifest arguments.
pub fn with_ffmpeg(ffmpeg: &Path) -> Vec<String> {
    let mut args = vec!["--ffmpeg".to_owned(), ffmpeg.display().to_string()];
    if let Some(ffprobe) = on_path("ffprobe") {
        args.extend(["--ffprobe".to_owned(), ffprobe.display().to_string()]);
    }
    args
}

/// What the real ffprobe says of `path`, as JSON.
pub fn ffprobe(path: &Path, extra: &[&str]) -> Value {
    let output = Command::new(on_path("ffprobe").unwrap())
        .args(["-v", "error", "-print_format", "json"])
        .args(extra)
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

/// The codec of the first audio stream of `path`, and the container's duration in seconds.
pub fn codec_and_duration(path: &Path) -> (String, f64) {
    let json = ffprobe(path, &["-show_format", "-show_streams"]);
    let audio = json["streams"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["codec_type"] == "audio")
        .expect("an audio stream");
    let duration = json["format"]["duration"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    (audio["codec_name"].as_str().unwrap().to_owned(), duration)
}
