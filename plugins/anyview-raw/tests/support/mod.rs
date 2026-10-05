//! A scratch desktop for the plugin's tests: XDG directories under a temporary folder, the shipped
//! manifest template filled in for the plugin built for this run, and stand-in shell scripts for
//! libheif's tools, so no test needs libheif or touches the real system.

#![allow(dead_code, clippy::unwrap_used)]

use anyview_core::FilePath;
use anyview_platform::{Env, discover};
use anyview_plugin::{Installed, Plugins};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// The plugin's executable, built for this test run.
pub const PLUGIN: &str = env!("CARGO_BIN_EXE_anyview-raw");

/// The manifest template the installer fills in.
const TEMPLATE: &str = include_str!("../../../../dist/plugins/anyview-raw.toml.in");

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
        let found = self.discover(args);
        found
            .installed()
            .iter()
            .find(|plugin| plugin.manifest.id.as_str() == "raw")
            .cloned()
            .expect("the manifest is discovered")
    }

    /// The registry discovery builds with the manifest installed.
    pub fn discover(&self, args: &[String]) -> Plugins {
        let folder = self.env.dirs.data.join("anyview/plugins");
        fs::create_dir_all(&folder).unwrap();
        let args: Vec<String> = args.iter().map(|arg| format!("{arg:?}")).collect();
        let text = TEMPLATE
            .replace("@PREFIX@/libexec/anyview/anyview-raw", PLUGIN)
            .replace("args = []", &format!("args = [{}]", args.join(", ")));
        fs::write(folder.join("raw.toml"), text).unwrap();
        let found = discover(&self.env);
        assert!(found.rejected.is_empty(), "{:?}", found.rejected);
        found.plugins
    }

    /// A path in the scratch folder.
    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    /// A stand-in file the plugin is asked about.
    pub fn raw(&self) -> PathBuf {
        let path = self.path("photo.cr2");
        fs::write(&path, b"II*\0\x08\0\0\0\0\0\0\0 not really").unwrap();
        path
    }

    /// A PNG of `width` x `height` the stand-ins write, left half red and right half blue.
    pub fn png(&self, name: &str, width: u32, height: u32) -> PathBuf {
        let image = image::RgbImage::from_fn(width, height, |x, _| {
            if x < width / 2 {
                image::Rgb([255, 0, 0])
            } else {
                image::Rgb([0, 0, 255])
            }
        });
        let path = self.path(name);
        image.save(&path).unwrap();
        path
    }

    /// An executable script named `name` with `body`.
    pub fn script(&self, name: &str, body: &str) -> PathBuf {
        let path = self.path(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        let mut perms = fs::metadata(&path).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
        fs::set_permissions(&path, perms).unwrap();
        path
    }

    /// A JPEG of `width` x `height` the stand-ins write as an embedded preview.
    pub fn jpeg(&self, name: &str, width: u32, height: u32) -> PathBuf {
        let image = image::RgbImage::from_fn(width, height, |_, _| image::Rgb([0, 200, 0]));
        let path = self.path(name);
        image.save(&path).unwrap();
        path
    }

    /// A PPM of `width` x `height` the stand-ins write as a development.
    pub fn ppm(&self, name: &str, width: u32, height: u32) -> PathBuf {
        self.png(&format!("{name}.png"), width, height);
        let image = image::open(self.path(&format!("{name}.png"))).unwrap();
        let path = self.path(name);
        image
            .save_with_format(&path, image::ImageFormat::Pnm)
            .unwrap();
        path
    }

    /// A stand-in for `dcraw_emu`: it runs `body` with the input, its last argument, in `$last`
    /// and the other arguments in `$*`.
    pub fn fake_emu(&self, body: &str) -> PathBuf {
        self.script("fake-dcraw-emu", &format!("for last; do :; done\n{body}"))
    }

    /// A stand-in for `dcraw`, which writes to standard output.
    pub fn fake_dcraw(&self, body: &str) -> PathBuf {
        self.script("fake-dcraw", &format!("for last; do :; done\n{body}"))
    }
}

pub fn with_emu(tool: &Path) -> Vec<String> {
    vec!["--dcraw-emu".to_owned(), tool.display().to_string()]
}

pub fn with_dcraw(tool: &Path) -> Vec<String> {
    vec!["--dcraw".to_owned(), tool.display().to_string()]
}

/// Whether the process whose id the file holds is gone, waiting a moment for it to be.
pub fn gone(pid_file: &Path) -> bool {
    let pid = fs::read_to_string(pid_file).unwrap().trim().to_owned();
    let proc = Path::new("/proc").join(pid);
    (0..100).any(|_| {
        if proc.exists() {
            std::thread::sleep(std::time::Duration::from_millis(30));
            false
        } else {
            true
        }
    })
}
