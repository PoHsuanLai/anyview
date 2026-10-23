//! An Extension that is installed, would serve a request, and is turned off.

use crate::handles::Subject;
use crate::provision::Provision;
use anyview_core::{Fact, FactLabel, FactValue};
use anyview_plugin_protocol::Capability;

/// An installed plugin the person turned off. Discovery sets it apart instead of ranking it, so
/// a lookup that finds nothing serving a kind can say "it is turned off" where it would have said
/// "it is missing" ([`Route::Off`](crate::Route::Off)).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OffPlugin {
    id: String,
    name: String,
    provides: Vec<Provision>,
}

impl OffPlugin {
    /// The plugin `id`, called `name`, that would provide `provides` if it were on.
    #[must_use]
    pub fn new(id: impl Into<String>, name: impl Into<String>, provides: Vec<Provision>) -> Self {
        OffPlugin {
            id: id.into(),
            name: name.into(),
            provides,
        }
    }

    /// Its Extension id.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// What a person calls it.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The `Needs` row that says it is turned off, as a facts card lists it.
    #[must_use]
    pub fn fact(&self) -> Fact {
        Fact::new(
            FactLabel::Needs,
            FactValue::text(format!("{} is turned off", self.name)),
        )
    }

    /// Whether it would serve `capability` for `subject` if it were on.
    #[must_use]
    pub fn serves(&self, capability: Capability, subject: &Subject<'_>) -> bool {
        self.provides.iter().any(|provision| {
            provision.capability() == capability && provision.handles().handles(subject)
        })
    }
}
