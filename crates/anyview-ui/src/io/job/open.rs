//! What a window and a worker hand each other around an open: the port a picture is uploaded to,
//! what a probe found, a preloaded file, and how a worker's results are kept.

use super::super::seams::{FileAccess, ImagePlugins};
use crate::StageFamily;
use crate::families::LoadedDoc;
use anyview_core::{Resume, Sniffed, Source};
use anyview_peek::StillSource;
use anyview_text::Highlighter;
use ds_blitz::TextureHandle;
use std::sync::Arc;

/// What an open needs from the window: the texture a picture is uploaded into (one per open, made
/// on the UI thread from the window's `Gpu`), the one highlighter every file shares, and the
/// pictures the host has ready.
#[derive(Debug, Clone)]
pub struct OpenPort {
    /// Where a decoded picture goes.
    pub texture: TextureHandle,
    /// Highlights code for every stage.
    pub highlighter: Arc<Highlighter>,
    /// The host's small pictures, for a first frame.
    pub first_frames: Arc<dyn StillSource>,
    /// The plugins that decode what the viewer cannot.
    pub image_plugins: Arc<dyn ImagePlugins>,
    /// The host's players, when this open may start one: a file opened ahead of the person never
    /// plays, so a preload carries none.
    pub(crate) media: Option<super::super::MediaPort>,
}

/// A file whose type was read: what to open, how it was sniffed, which stage shows it and where
/// the person left it last time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opened {
    /// The file and its stamp.
    pub source: Source,
    /// Proof the type was sniffed.
    pub sniffed: Sniffed,
    /// The stage that shows it.
    pub family: StageFamily,
    /// Where it was left, as the host's store remembered it for this version of the file.
    pub resume: Resume,
    /// Whether the file takes a save in place.
    pub access: FileAccess,
}

/// A file opened ahead of the person asking for it.
#[derive(Debug, Clone)]
pub struct Preloaded {
    /// What was probed.
    pub probed: Opened,
    /// The open document.
    pub doc: LoadedDoc,
}

/// How soon a job is wanted, for a pool that runs the nearest first: what the person is looking
/// at now is `Visible`, what is read ahead of them is `Preload`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkLane {
    /// Wanted now.
    Visible,
    /// Wanted soon, if at all.
    Preload,
}

/// What a window has to keep of the person's views, written by a worker through a [`ResumeKeeper`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Keeping {
    /// This file was shown.
    Viewed(Opened),
    /// The person is here in this file.
    Place {
        /// Where the file is from.
        source: Source,
        /// The place the person is at.
        resume: Resume,
    },
}

/// What a newer job of the same load replaces, and so what stop it runs under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::io) enum Replaces {
    /// No older job: the job runs under the load's own stop.
    Nothing,
    /// The older search of the load.
    Search,
    /// The older read of a window of lines of the load.
    Lines,
}
