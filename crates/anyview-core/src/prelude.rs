//! The names a consumer of the vocabulary uses in nearly every file: what a file is called and
//! where it is, how its bytes are read, what it was sniffed to be, and the kinds, actions and
//! facts that come out. `use anyview_core::prelude::*;` brings them in.
//!
//! What belongs here is a type a caller names to *start* (an [`Input`], a [`Source`], a
//! [`PeekBudget`]) or to read an answer (a [`FormatKind`], [`Facts`]). Units, export options, the
//! sequence and the trail are reached from the crate root by the code that needs them.

pub use crate::{
    Facts, FileAction, FileHead, FileName, FilePath, FileStamp, FormatKind, Input, PeekBudget,
    ReadAt, Resume, Sniffed, Source, sniff,
};
