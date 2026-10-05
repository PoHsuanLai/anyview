//! View memory: where a person left a file, so opening it again continues there.

use crate::units::{
    DocPoint, LineIndex, MediaTime, PageIndex, Permille, SectionIndex, Volume, Zoom,
};

/// A track of audio or subtitles in a recording, by its number in the container.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct TrackId(pub u32);

/// Which track plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum TrackChoice {
    /// The player's own choice (the default track, or the one that matches the language).
    Auto,
    /// No track.
    Off,
    /// This track.
    Track(TrackId),
}

/// Where a person left a file. Stored per file, so it is adjacently tagged (CONVENTIONS section 12).
/// a kind the viewer no longer has still loads.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum Resume {
    /// An image: its zoom and the point at the centre of the window.
    Raster {
        /// How large it was drawn.
        zoom: Zoom,
        /// The point in the image at the window's centre.
        centre: DocPoint,
    },
    /// A PDF: the page at the top, how far down it, and the zoom.
    Pdf {
        /// The page at the top of the window.
        page: PageIndex,
        /// How far down the page the window starts, in thousandths of the page's height.
        offset: Permille,
        /// How large it was drawn.
        zoom: Zoom,
    },
    /// A recording: the position, the level and the chosen tracks.
    Media {
        /// The playback position.
        at: MediaTime,
        /// The output level.
        volume: Volume,
        /// The audio track.
        audio: TrackChoice,
        /// The subtitle track.
        subtitles: TrackChoice,
    },
    /// A text file: the first visible line.
    Text {
        /// The line at the top of the window.
        line: LineIndex,
    },
    /// A book: the chapter or comic page being read.
    Book {
        /// The section on screen.
        section: SectionIndex,
    },
    /// Nothing worth restoring.
    Nothing,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::{DocUnit, Percent};

    fn cases() -> Vec<(&'static str, Resume, &'static str)> {
        vec![
            (
                "raster",
                Resume::Raster {
                    zoom: Zoom::Scale(Permille(2000)),
                    centre: DocPoint {
                        x: DocUnit(64),
                        y: DocUnit(-128),
                    },
                },
                r#"{"kind":"raster","v":{"zoom":{"kind":"scale","v":2000},"centre":{"x":64,"y":-128}}}"#,
            ),
            (
                "pdf",
                Resume::Pdf {
                    page: PageIndex(11),
                    offset: Permille(250),
                    zoom: Zoom::Fit,
                },
                r#"{"kind":"pdf","v":{"page":11,"offset":250,"zoom":{"kind":"fit"}}}"#,
            ),
            (
                "media",
                Resume::Media {
                    at: MediaTime::from_secs(90),
                    volume: Volume::clamped(Percent(80)),
                    audio: TrackChoice::Track(TrackId(2)),
                    subtitles: TrackChoice::Off,
                },
                r#"{"kind":"media","v":{"at":90000000,"volume":80,"audio":{"kind":"track","v":2},"subtitles":{"kind":"off"}}}"#,
            ),
            (
                "text",
                Resume::Text {
                    line: LineIndex(480),
                },
                r#"{"kind":"text","v":{"line":480}}"#,
            ),
            (
                "book",
                Resume::Book {
                    section: SectionIndex(7),
                },
                r#"{"kind":"book","v":{"section":7}}"#,
            ),
            ("nothing", Resume::Nothing, r#"{"kind":"nothing"}"#),
        ]
    }

    #[test]
    fn every_resume_round_trips_in_its_stored_form() {
        for (name, resume, json) in cases() {
            assert_eq!(
                serde_json::to_string(&resume).unwrap(),
                json,
                "{name} writes"
            );
            assert_eq!(
                serde_json::from_str::<Resume>(json).unwrap(),
                resume,
                "{name} reads"
            );
        }
    }

    #[test]
    fn a_stored_volume_above_the_limit_loads_clamped() {
        let json = r#"{"kind":"media","v":{"at":0,"volume":900,"audio":{"kind":"auto"},"subtitles":{"kind":"auto"}}}"#;
        let Resume::Media { volume, .. } = serde_json::from_str::<Resume>(json).unwrap() else {
            panic!("not a media resume");
        };
        assert_eq!(volume, Volume::MAX);
    }

    #[test]
    fn track_choices_round_trip() {
        const CASES: &[(TrackChoice, &str)] = &[
            (TrackChoice::Auto, r#"{"kind":"auto"}"#),
            (TrackChoice::Off, r#"{"kind":"off"}"#),
            (TrackChoice::Track(TrackId(7)), r#"{"kind":"track","v":7}"#),
        ];
        for (choice, json) in CASES {
            assert_eq!(&serde_json::to_string(choice).unwrap(), json);
            assert_eq!(&serde_json::from_str::<TrackChoice>(json).unwrap(), choice);
        }
    }
}
