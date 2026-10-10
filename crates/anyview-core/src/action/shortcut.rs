//! The keys of an action.

use super::FileAction;
use super::spec::{Binding, spec_of};
use ds_core::vocab::Shortcut;

/// The shortcut of `action`, or `None` when it has none.
///
/// An action that means what a standard action means (open, copy, print, save a copy) carries
/// that standard action's keys. The rest are the viewer's own, built through `Shortcut::custom`,
/// which refuses a reserved combination: a clash is a missing shortcut, never a repurposed one.
#[must_use]
pub fn shortcut(action: FileAction) -> Option<Shortcut> {
    match spec_of(action).binding {
        Binding::Unbound => None,
        Binding::Standard(standard) => Some(Shortcut::standard(standard)),
        Binding::Own(keys) => Shortcut::custom(keys.iter().copied()).ok(),
    }
}
