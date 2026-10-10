//! A recording in a pane: it plays in the pane when the host gave the edge a player, and is the
//! host's to open elsewhere when it did not.

use crate::support::{Host, hosted_with, media_file};
use anyview_core::{Facts, FilePath, MediaTags};
use anyview_pane::{MediaHost, PaneRequest};
use anyview_ui::{
    MediaLine, MediaNotice, MediaOffer, MediaPlayback, MediaStart, MediaStarted, OpenError,
    PlayerCommand, SlotPixels,
};
use ds_harness::{Query, Viewport};
use std::path::Path;
use std::sync::Arc;

/// A player that starts and says nothing: enough for a pane to show the stage of a playing file.
#[derive(Debug)]
struct Silent;

impl MediaLine for Silent {
    fn send(&self, _command: PlayerCommand) {}

    fn resize(&self, _slot: Option<SlotPixels>) {}

    fn drain(&self) -> Vec<MediaNotice> {
        Vec::new()
    }
}

#[derive(Debug)]
struct Plays;

impl MediaHost for Plays {
    fn start(&self, _start: MediaStart) -> Result<MediaStarted, OpenError> {
        Ok(MediaStarted {
            playback: MediaPlayback::Line(Arc::new(Silent)),
            offer: MediaOffer::new(Vec::new(), None),
            tags: MediaTags::default(),
            facts: Facts::empty(),
            length: None,
            cover: None,
        })
    }
}

fn in_a_corner() -> Viewport {
    Viewport {
        width: 640,
        height: 400,
        scale_percent: 100,
    }
}

fn open_in_a_pane(file: &Path, media: Option<Arc<dyn MediaHost>>) -> Host {
    hosted_with(vec![(file.to_path_buf(), None)], in_a_corner(), None, media)
}

#[test]
fn a_recording_plays_in_the_pane_only_where_the_host_gave_it_a_player() {
    let dir = tempfile::tempdir().unwrap();
    let file = media_file(dir.path(), "clip.mkv", "clip.mkv");
    let handed = PaneRequest::OpenElsewhere(vec![FilePath::new(&file).unwrap()]);
    let player: Arc<dyn MediaHost> = Arc::new(Plays);
    /// Name, the host's player, the media stage shows, the pane hands the file to its host.
    type Row = (&'static str, Option<Arc<dyn MediaHost>>, usize, bool);
    let rows: Vec<Row> = vec![
        ("with a player it plays here", Some(player), 1, false),
        ("with none it is the host's to open", None, 0, true),
    ];
    for (name, media, stage, elsewhere) in rows {
        let host = open_in_a_pane(&file, media);
        assert_eq!(host.harness.count(".viewer-media"), stage, "{name}: stage");
        assert_eq!(host.asked(&handed), elsewhere, "{name}: handed to the host");
    }
}
