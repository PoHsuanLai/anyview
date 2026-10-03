//! The routing seam: where a kind with no built-in back end looks for a plugin.

use super::runner::PluginRunner;
use crate::error::PlatformError;
use anyview_core::{FactLabel, FactValue, Facts, FilePath};
use anyview_plugin::{MissingPlugin, Plugins, Route, Subject};
use anyview_plugin_protocol::{Capability, FactRow};
use ds_core::word::Word;

/// What asking the plugins about a file came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginFacts {
    /// A plugin read these rows.
    Facts(Facts),
    /// No installed plugin serves the kind; this package would.
    Missing(MissingPlugin),
    /// No plugin serves it and none is known to: the caller shows what it already has.
    Unserved,
}

impl PluginRunner {
    /// The facts of `path`, a file of `subject`'s kind that no built-in back end reads, from
    /// the plugin `plugins` routes `probe` to. When none serves it, the answer says which
    /// package would. A plugin that fails is an error the caller shows or ignores; the viewer
    /// is never taken down with it. Blocking.
    pub fn peek_facts(
        &self,
        plugins: &Plugins,
        subject: &Subject<'_>,
        path: &FilePath,
    ) -> Result<PluginFacts, PlatformError> {
        match plugins.route(Capability::Probe, subject) {
            Route::Served(plugin) => {
                let rows = self.probe(plugin, path)?;
                Ok(PluginFacts::Facts(facts_of(&rows)))
            }
            Route::Missing(missing) => Ok(PluginFacts::Missing(missing)),
            Route::Unserved => Ok(PluginFacts::Unserved),
        }
    }
}

/// The rows a plugin sent as `Facts`, dropping a label the viewer does not know.
fn facts_of(rows: &[FactRow]) -> Facts {
    rows.iter()
        .filter_map(|row| Some((FactLabel::parse(&row.label)?, &row.value)))
        .fold(Facts::empty(), |facts, (label, value)| {
            facts.with(label, FactValue::text(value.clone()))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_labels_are_dropped_and_order_is_kept() {
        let row = |label: &str, value: &str| FactRow {
            label: label.to_owned(),
            value: value.to_owned(),
        };
        let facts = facts_of(&[row("codec", "h264"), row("mood", "blue"), row("title", "T")]);
        let labels: Vec<_> = facts.rows().iter().map(|fact| fact.label).collect();
        assert_eq!(labels, [FactLabel::Codec, FactLabel::Title]);
    }
}
