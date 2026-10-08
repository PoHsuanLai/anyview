//! What a capability handles, and what is asked of it.

use crate::error::ProvisionFault;
use anyview_core::{FormatKind, Mime};
use anyview_plugin_protocol::Capability;

/// The kinds and MIME types a capability handles. Never both empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handles {
    kinds: Vec<FormatKind>,
    mimes: Vec<Mime>,
}

impl Handles {
    /// What `capability` handles, or `HandlesNothing` when both lists are empty.
    pub fn new(
        capability: Capability,
        kinds: Vec<FormatKind>,
        mimes: Vec<Mime>,
    ) -> Result<Handles, ProvisionFault> {
        if kinds.is_empty() && mimes.is_empty() {
            return Err(ProvisionFault::HandlesNothing { capability });
        }
        Ok(Handles { kinds, mimes })
    }

    /// The kinds, as written.
    pub fn kinds(&self) -> &[FormatKind] {
        &self.kinds
    }

    /// The MIME types, as written.
    pub fn mimes(&self) -> &[Mime] {
        &self.mimes
    }

    /// Whether `subject` is one of the kinds or one of the MIME types.
    pub fn handles(&self, subject: &Subject<'_>) -> bool {
        self.names_kind(subject.kind) || self.names_mime(subject)
    }

    /// Whether the subject's own MIME type is listed: a closer match than its kind.
    pub(crate) fn names_mime(&self, subject: &Subject<'_>) -> bool {
        subject.mime.is_some_and(|mime| self.mimes.contains(mime))
    }

    fn names_kind(&self, kind: FormatKind) -> bool {
        self.kinds.contains(&kind)
    }
}

/// What a request is about: a file's kind and, when sniffing knew it, its MIME type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Subject<'a> {
    /// The kind.
    pub kind: FormatKind,
    /// The MIME type, when known.
    pub mime: Option<&'a Mime>,
}
