//! A registry of plugins for the tests of the crates that consume it: the manifests the installer
//! writes, filled in for programs the test names, with none of the checks that the programs exist.

use anyview_plugin::{Candidate, Manifest, Origin, Plugins, Readiness};
use std::path::Path;

/// Which plugins a test has installed.
#[derive(Debug, Clone, Copy, Default)]
pub struct PluginSet<'a> {
    /// The `mpv` and the C plugin that play, when playing is installed.
    pub play: Option<(&'a Path, &'a Path)>,
    /// The FFmpeg plugin's program, when it is installed.
    pub ffmpeg: Option<&'a Path>,
    /// What its manifest adds to the program's command line (`--ffmpeg PATH`).
    pub ffmpeg_args: &'a [String],
}

/// The registry `set` makes.
pub fn plugins_with(set: PluginSet<'_>) -> Plugins {
    let mut candidates = Vec::new();
    if let Some((mpv, cplugin)) = set.play {
        let text = format!(
            "id = \"mpv\"\nname = \"mpv\"\nprotocol = 1\n[[provides]]\ncapability = \"play\"\n\
             kinds = [\"video\", \"audio\"]\nmpv = {:?}\ncplugin = {:?}\n",
            mpv.display().to_string(),
            cplugin.display().to_string()
        );
        candidates.push(candidate(&text));
    }
    if let Some(program) = set.ffmpeg {
        let mut text = format!(
            "id = \"ffmpeg\"\nname = \"FFmpeg\"\nprotocol = 1\n[program]\npath = {:?}\nargs = {:?}\n",
            program.display().to_string(),
            set.ffmpeg_args
        );
        for capability in ["probe", "peek", "thumbnail", "decode"] {
            text.push_str(&format!(
                "[[provides]]\ncapability = \"{capability}\"\nkinds = [\"video\", \"audio\"]\n"
            ));
        }
        text.push_str(
            "[[provides]]\ncapability = \"export\"\nkinds = [\"video\", \"audio\"]\n\
             targets = [\"trim\", \"audio-copy\", \"m4a\", \"mp3\", \"flac\", \"wav\", \"opus\"]\n",
        );
        candidates.push(candidate(&text));
    }
    Plugins::resolve(candidates)
}

fn candidate(text: &str) -> Candidate {
    Candidate {
        manifest: Manifest::parse(text)
            .unwrap_or_else(|error| panic!("the test's manifest is not valid ({error}): {text}")),
        origin: Origin::User,
        readiness: Readiness::Ready,
    }
}
