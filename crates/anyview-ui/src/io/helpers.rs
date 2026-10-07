//! A tool the viewer's plugins run and the person's system lacks, as the window learns of it: the
//! `Needs` row with the tool that would fix it, and the words the install sheet says. The host
//! owns the catalog of tools and the installing; the window only asks, and hears how it ended.

use anyview_core::{Fact, Helper};
use std::fmt::Debug;

/// A `Needs` row, and the tool the viewer can offer to install to answer it. The row is the
/// passive state; the tool, when there is one, is what an Install… button on it asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Need {
    /// The row a facts card lists: what the person would install, and what it is for.
    pub fact: Fact,
    /// The tool whose installation would answer it, or `None` when the viewer cannot install one
    /// (the plugin itself is absent, or the system has no way to install).
    pub helper: Option<Helper>,
}

impl Need {
    /// A row that only says what is missing: no install is offered for it.
    pub fn passive(fact: Fact) -> Need {
        Need { fact, helper: None }
    }
}

/// What the install sheet says of one tool: "{app} needs {tool} to {purpose}."
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelperWords {
    /// The app's name.
    pub app: String,
    /// The tool, as the person knows it.
    pub tool: String,
    /// What it is for, finishing the sentence ("open HEIC photos").
    pub purpose: String,
    /// The package to look for in a software centre when the sources have none.
    pub package: String,
    /// The program a package must provide when the system cannot install one from here.
    pub program: String,
}

/// The tools the host knows how to install, lent to the window to word its sheet.
pub trait HelperSource: Debug + Send + Sync + 'static {
    /// The words for `helper`, or `None` when the host does not know the tool.
    fn words(&self, helper: Helper) -> Option<HelperWords>;
}

/// The default `HelperSource`: the host installs nothing, so the sheet is never worded.
#[derive(Debug, Clone, Copy)]
pub(crate) struct NoHelpers;

impl HelperSource for NoHelpers {
    fn words(&self, _helper: Helper) -> Option<HelperWords> {
        None
    }
}
