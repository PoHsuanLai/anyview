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

    fn root(&self, place: Where) -> PathBuf {
        match place {
            Where::User => self.env.dirs.data.clone(),
            Where::System => self.env.dirs.data_dirs[0].clone(),
        }
    }

    /// Writes `text` as the manifest of the Extension folder `folder` of `place`
    /// (`quire/extensions/<folder>/extension.toml`); the folder's name need not be the id inside.
    pub fn write_extension(&self, place: Where, folder: &str, text: &str) -> PathBuf {
        let folder = self.root(place).join("quire/extensions").join(folder);
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join("extension.toml");
        fs::write(&path, text).unwrap();
        path
    }

    /// Writes `text` as `<file>` in the old flat plugin folder of `place` (`anyview/plugins`),
    /// which discovery still reads for one release.
    pub fn write_legacy(&self, place: Where, file: &str, text: &str) -> PathBuf {
        let folder = self.root(place).join("anyview/plugins");
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join(file);
        fs::write(&path, text).unwrap();
        path
    }

    /// An Extension for the fake plugin under `id`, passing `args`, speaking `protocol`.
    pub fn install(&self, place: Where, id: &str, protocol: u32, args: &[&str]) -> PathBuf {
        self.write_extension(place, id, &fake_manifest(id, protocol, args))
    }

    /// The same plugin as an old flat manifest, `anyview/plugins/<id>.toml`.
    pub fn install_legacy(&self, place: Where, id: &str, protocol: u32, args: &[&str]) -> PathBuf {
        self.write_legacy(
            place,
            &format!("{id}.toml"),
            &fake_manifest(id, protocol, args),
        )
    }

    /// Turns the Extensions `ids` off, as Settings does: `quire/extensions.toml` in the config
    /// directory.
    pub fn turn_off(&self, ids: &[&str]) {
        let rows: String = ids
            .iter()
            .map(|id| format!("[[extensions.off]]\nid = \"{id}\"\n"))
            .collect();
        let folder = self.env.dirs.config.join("quire");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("extensions.toml"), rows).unwrap();
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
