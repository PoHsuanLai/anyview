//! The small values a family and the window trade: a finished edit, the bytes of a save, and a
//! shared handle on a document.

use crate::StageIn;
use crate::io::Job;
use anyview_text::EditStamp;
use dioxus::prelude::*;
use std::fmt::Debug;
use std::sync::Arc;

/// What a document did with the edits its stage asked for.
#[derive(Debug, Default)]
pub struct EditDone {
    /// What the stage is told: the edits as they stand, and where the cursor goes.
    pub tell: Vec<StageIn>,
    /// What is to be written over the file, when the edit was a save.
    pub write: SaveBytes,
}
/// What a save of a document's edits comes to.
#[derive(Debug, Default)]
pub enum SaveBytes {
    /// No save was asked.
    #[default]
    Nothing,
    /// These are the file's new bytes; the state of the edits they are is told with them.
    Bytes(Vec<u8>, EditStamp),
    /// A worker builds the bytes: the job is for the workers.
    Building(Job),
}
/// A shared piece of a document or window state that compares by identity.
#[derive(Debug)]
pub struct Held<T>(pub Arc<T>);
impl<T> Clone for Held<T> {
    fn clone(&self) -> Self {
        Held(Arc::clone(&self.0))
    }
}
impl<T> PartialEq for Held<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
