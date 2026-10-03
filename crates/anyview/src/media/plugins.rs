//! What the program knows of its plugins: the registry built once from the person's data
//! directories, and the questions the media code asks it. Playing, probing and exporting a
//! recording are all done by programs the person installed; this is where the viewer finds out
//! which, and what to say when there are none.

use anyview_media::MpvHost;
use anyview_platform::PluginRunner;
use anyview_plugin::{MissingPlugin, Plugins, Provision, Route, Subject};
use anyview_plugin_protocol::Capability;

/// The plugins of this run and the runner that talks to them.
#[derive(Debug, Clone, Default)]
pub struct MediaPlugins {
    plugins: Plugins,
    runner: PluginRunner,
}

/// What asking for a player came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayRoute {
    /// A plugin plays it, with these programs.
    Ready(MpvHost),
    /// None does; this package would.
    Missing(MissingPlugin),
    /// None does, and no package is known to.
    Unserved,
}

impl MediaPlugins {
    /// The plugins `plugins` holds, run by `runner`.
    pub fn new(plugins: Plugins, runner: PluginRunner) -> MediaPlugins {
        MediaPlugins { plugins, runner }
    }

    /// The registry.
    pub fn plugins(&self) -> &Plugins {
        &self.plugins
    }

    /// The runner.
    pub fn runner(&self) -> &PluginRunner {
        &self.runner
    }

    /// The programs that play a file of `subject`'s kind.
    pub fn player(&self, subject: &Subject<'_>) -> PlayRoute {
        match self.plugins.route(Capability::Play, subject) {
            Route::Served(plugin) => match plugin.manifest.provision(Capability::Play) {
                Some(Provision::Play(play)) => PlayRoute::Ready(MpvHost {
                    mpv: play.player.mpv.clone(),
                    cplugin: play.player.cplugin.clone(),
                }),
                Some(
                    Provision::Probe(_)
                    | Provision::Peek(_)
                    | Provision::Thumbnail(_)
                    | Provision::Decode(_)
                    | Provision::Export(_),
                )
                | None => PlayRoute::Unserved,
            },
            Route::Missing(missing) => PlayRoute::Missing(missing),
            Route::Unserved => PlayRoute::Unserved,
        }
    }
}
