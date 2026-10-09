//! A scratch desktop for the plugin tests: XDG directories under a temporary folder, manifests
//! for the fake plugin written into them, and a file for it to read. Nothing here reaches the
//! real home directory or the real plugins.

#![allow(dead_code)]

use anyview_core::{FilePath, FormatKind};
use anyview_platform::{Env, discover};
use anyview_plugin::{Installed, Plugins, Subject};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// The fake plugin's executable, built for this test run.
pub const FAKE: &str = env!("CARGO_BIN_EXE_anyview-fake-plugin");

/// Which directory a manifest is installed in.
#[derive(Debug, Clone, Copy)]
pub enum Where {
    /// `$XDG_DATA_HOME`.
    User,
    /// The first of `$XDG_DATA_DIRS`.
    System,
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

    fn plugins_dir(&self, place: Where) -> PathBuf {
        let root = match place {
            Where::User => self.env.dirs.data.clone(),
            Where::System => self.env.dirs.data_dirs[0].clone(),
        };
        let folder = root.join("anyview/plugins");
        fs::create_dir_all(&folder).unwrap();
        folder
    }

    /// Writes `text` as `<file>` in the plugin folder of `place`.
    pub fn write_manifest(&self, place: Where, file: &str, text: &str) -> PathBuf {
        let path = self.plugins_dir(place).join(file);
        fs::write(&path, text).unwrap();
        path
    }

    /// A manifest for the fake plugin under `id`, passing `args`, speaking `protocol`.
    pub fn install(&self, place: Where, id: &str, protocol: u32, args: &[&str]) -> PathBuf {
        self.write_manifest(
            place,
            &format!("{id}.toml"),
            &fake_manifest(id, protocol, args),
        )
    }

    /// A text file whose first line is `line`.
    pub fn book(&self, name: &str, line: &str) -> FilePath {
        let path = self.dir.path().join(name);
        fs::write(&path, format!("{line}\nmore\n")).unwrap();
        FilePath::new(path).unwrap()
    }

    /// The plugin `id`, discovered.
    pub fn plugin(&self, id: &str) -> Installed {
        let plugins = discover(&self.env).plugins;
        plugins
            .installed()
            .iter()
            .find(|plugin| plugin.manifest.id.as_str() == id)
            .cloned()
            .unwrap_or_else(|| panic!("plugin {id} was not discovered"))
    }

    pub fn plugins(&self) -> Plugins {
        discover(&self.env).plugins
    }
}

pub fn book() -> Subject<'static> {
    Subject {
        kind: FormatKind::Book,
        mime: None,
    }
}

/// The manifest of the fake plugin: every wire capability for the kind `book`.
pub fn fake_manifest(id: &str, protocol: u32, args: &[&str]) -> String {
    let args: Vec<String> = args.iter().map(|arg| format!("\"{arg}\"")).collect();
    format!(
        r#"id = "{id}"
name = "Fake {id}"
protocol = {protocol}

[program]
path = "{FAKE}"
args = [{}]

[[provides]]
capability = "probe"
kinds = ["book"]

[[provides]]
capability = "thumbnail"
kinds = ["book"]

[[provides]]
capability = "decode"
kinds = ["book"]

[[provides]]
capability = "export"
kinds = ["book"]
targets = ["txt"]
"#,
        args.join(", ")
    )
}
