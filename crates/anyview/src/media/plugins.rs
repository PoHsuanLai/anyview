//! What the program knows of its plugins: the registry built once from the person's data
//! directories, and the questions the media code asks it. Playing, probing and exporting a
//! recording are all done by programs the person installed; this is where the viewer finds out
//! which, and what to say when there are none.

use crate::host::{HelperHost, PluginRegistry};
use anyview_core::{
    ByteLen, Fact, FactLabel, FactValue, Facts, FormatKind, Helper, MediaTags, PeekBudget,
    PixelArea, Sniffed, Source,
};
use anyview_media::{MpvHost, offered_kinds, target_of};
use anyview_peek::{AudioCover, audio_cover};
use anyview_platform::{PluginFacts, PluginRunner};
use anyview_plugin::{Installed, MissingPlugin, Plugins, Provision, Route, Subject};
use anyview_plugin_protocol::Capability;
use anyview_ui::{MediaOffer, Need};
use std::sync::Arc;
use std::time::Duration;

/// What the pure-Rust reader of a recording's header may spend when no plugin reads it.
const HEADER_BUDGET: PeekBudget = PeekBudget {
    bytes: ByteLen(16 << 20),
    pixels: PixelArea(1_000_000),
    time: Duration::from_secs(2),
};

/// The plugins of this run and the runner that talks to them.
#[derive(Debug, Clone, Default)]
pub struct MediaPlugins {
    registry: PluginRegistry,
    runner: PluginRunner,
    helpers: Option<Arc<HelperHost>>,
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
        MediaPlugins::following(PluginRegistry::fixed(plugins), runner)
    }

    /// The plugins `registry` holds now (it may read them again), run by `runner`.
    pub fn following(registry: PluginRegistry, runner: PluginRunner) -> MediaPlugins {
        MediaPlugins {
            registry,
            runner,
            helpers: None,
        }
    }

    /// The same plugins, offering to install the tools `helpers` knows when one is missing.
    pub fn offering(self, helpers: Arc<HelperHost>) -> MediaPlugins {
        MediaPlugins {
            helpers: Some(helpers),
            ..self
        }
    }

    /// The registry as it is now.
    pub fn plugins(&self) -> Arc<Plugins> {
        self.registry.current()
    }

    /// The row that says what would play, or read, a recording `missing` names, with the tool to
    /// offer to install when the plugin is installed and the system's tool is what is absent (a
    /// plugin that is itself absent is not something to install a tool for).
    pub fn need_of(&self, missing: &MissingPlugin) -> Need {
        let fact = missing.fact();
        match &self.helpers {
            Some(helpers) if self.plugins().tool_absent(missing.package) => {
                helpers.need(fact, missing.package.helper())
            }
            Some(_) | None => Need::passive(fact),
        }
    }

    /// The runner.
    pub fn runner(&self) -> &PluginRunner {
        &self.runner
    }

    /// The programs that play a file of `subject`'s kind.
    pub fn player(&self, subject: &Subject<'_>) -> PlayRoute {
        let plugins = self.plugins();
        match plugins.route(Capability::Play, subject) {
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

/// A recording's facts and tags, from whoever could read them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reading {
    /// The rows beyond the kind and the size, which the window adds itself.
    pub facts: Facts,
    /// The title, artist and album the recording says.
    pub tags: MediaTags,
    /// The cover an audio file carries, read by the pure-Rust reader of its header (a plugin's
    /// facts have no picture).
    pub cover: Option<AudioCover>,
}

/// Which program writes a recording's exports.
#[derive(Debug, Clone)]
pub struct ExportTool {
    /// The runner that talks to it.
    pub runner: PluginRunner,
    /// The plugin that writes.
    pub plugin: Installed,
}

/// What asking for a writer came to.
#[derive(Debug, Clone)]
pub enum WriteRoute {
    /// A plugin writes it.
    Ready(Arc<ExportTool>),
    /// None does; this package would.
    Missing(MissingPlugin),
    /// None does, and no package is known to.
    Unserved,
}

impl MediaPlugins {
    /// What the recording `source` says of itself: the FFmpeg plugin's facts when it is installed
    /// and answers, else what the header says to a pure-Rust reader. A plugin that fails is said
    /// and set aside; the recording still opens.
    pub fn reading(&self, source: &Source, sniffed: &Sniffed) -> Reading {
        let subject = Subject {
            kind: sniffed.kind(),
            mime: Some(sniffed.mime()),
        };
        let facts = match self
            .runner
            .peek_facts(&self.plugins(), &subject, source.path())
        {
            Ok(PluginFacts::Facts(facts)) => facts,
            Ok(PluginFacts::Missing(_) | PluginFacts::Unserved) => header_facts(source, sniffed),
            Err(error) => {
                eprintln!("anyview: the plugin could not read the recording: {error}");
                header_facts(source, sniffed)
            }
        };
        let text = |label: FactLabel| facts.value(label).map(|value| value.as_str().to_owned());
        let tags = MediaTags {
            title: text(FactLabel::Title),
            artist: text(FactLabel::Author),
            album: text(FactLabel::Album),
        };
        let cover = (sniffed.kind() == FormatKind::Audio)
            .then(|| audio_cover(source, sniffed, &HEADER_BUDGET))
            .flatten();
        Reading { facts, tags, cover }
    }

    /// The writer of `subject`'s exports.
    pub fn writer(&self, subject: &Subject<'_>) -> WriteRoute {
        match self.plugins().route(Capability::Export, subject) {
            Route::Served(plugin) => WriteRoute::Ready(Arc::new(ExportTool {
                runner: self.runner,
                plugin: plugin.clone(),
            })),
            Route::Missing(missing) => WriteRoute::Missing(missing),
            Route::Unserved => WriteRoute::Unserved,
        }
    }

    /// The exports on offer for a recording of `sniffed`'s kind: the formats the installed writer
    /// says this machine can write (its greeting, since an FFmpeg may lack an encoder the plugin
    /// knows), and the frame of a video when a player shows it. What is missing is named by the
    /// package that adds it.
    pub fn offer(&self, sniffed: &Sniffed, playing: Playing) -> MediaOffer {
        let subject = Subject {
            kind: sniffed.kind(),
            mime: Some(sniffed.mime()),
        };
        let writable = match self.writer(&subject) {
            WriteRoute::Ready(tool) => self.greeted(&tool),
            WriteRoute::Missing(_) | WriteRoute::Unserved => None,
        };
        let kinds: Vec<_> = offered_kinds(sniffed.kind())
            .into_iter()
            .filter(|kind| match target_of(*kind) {
                Some(target) => writable
                    .as_ref()
                    .is_some_and(|targets| targets.iter().any(|name| name == target)),
                None => playing == Playing::Yes,
            })
            .collect();
        let plugins = self.plugins();
        let route = plugins.route(Capability::Export, &subject);
        let needs = match (&writable, &route) {
            (Some(_), _) => None,
            (None, Route::Missing(missing)) => Some(missing.fact()),
            (None, Route::Served(_) | Route::Unserved) => Some(Fact {
                label: FactLabel::Needs,
                value: FactValue::text("a working FFmpeg for the FFmpeg plugin (to convert it)"),
            }),
        };
        let offer = MediaOffer::new(kinds, needs);
        // The FFmpeg plugin is installed and its FFmpeg is what is missing: that is a tool to
        // install. A plugin that is itself absent is not.
        match (&self.helpers, &writable, &route) {
            (Some(helpers), None, Route::Served(_)) if helpers.offers(Helper::MediaProbe) => {
                offer.installable(Helper::MediaProbe)
            }
            (Some(_) | None, _, _) => offer,
        }
    }

    /// The targets the writer can write now, from its greeting; `None` when it could not be
    /// asked or says it writes nothing.
    fn greeted(&self, tool: &ExportTool) -> Option<Vec<String>> {
        let hello = match self.runner.hello(&tool.plugin) {
            Ok(hello) => hello,
            Err(error) => {
                eprintln!("anyview: the plugin that writes exports did not answer: {error}");
                return None;
            }
        };
        if !hello.provides.contains(&Capability::Export) {
            return None;
        }
        let manifest = match tool.plugin.manifest.provision(Capability::Export) {
            Some(Provision::Export(export)) => export,
            Some(
                Provision::Probe(_)
                | Provision::Peek(_)
                | Provision::Thumbnail(_)
                | Provision::Decode(_)
                | Provision::Play(_),
            )
            | None => return None,
        };
        let listed = manifest.targets.iter().map(|name| name.as_str());
        let writable: Vec<String> = if hello.targets.is_empty() {
            listed.map(str::to_owned).collect()
        } else {
            listed
                .filter(|name| hello.targets.iter().any(|said| said == name))
                .map(str::to_owned)
                .collect()
        };
        (!writable.is_empty()).then_some(writable)
    }
}

/// Whether a player shows the recording's picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Playing {
    /// A player was started for it.
    Yes,
    /// None was.
    No,
}

/// What a pure-Rust reader of the header says, without the rows the window adds itself.
fn header_facts(source: &Source, sniffed: &Sniffed) -> Facts {
    let peeked = anyview_peek::peek(source, sniffed, &HEADER_BUDGET);
    peeked
        .facts
        .rows()
        .iter()
        .filter(|fact| {
            !matches!(
                fact.label,
                FactLabel::Kind | FactLabel::Size | FactLabel::Modified | FactLabel::Needs
            )
        })
        .fold(Facts::empty(), |facts, fact| {
            facts.with(fact.label, fact.value.clone())
        })
}
