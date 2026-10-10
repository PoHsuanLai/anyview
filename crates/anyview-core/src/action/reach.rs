//! Where an action may appear.

use super::FileAction;
use super::spec::spec_of;
use ds_core::word::Word;

/// The surfaces an action may appear on: the launcher's row menu, the viewer, or both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Word)]
pub enum Reach {
    /// The launcher only (the viewer already has the file open).
    Launcher,
    /// The viewer only (it needs the content in front of it).
    Viewer,
    /// Both.
    Both,
}

/// Where `action` may appear.
#[must_use]
pub fn reach(action: FileAction) -> Reach {
    spec_of(action).reach
}
