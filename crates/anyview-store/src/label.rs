//! `ResumeLabel`: the short "where I left it" a history row shows, derived from a `Resume`.

use anyview_core::Resume;
use std::fmt;

/// Where a person left a file, in the words a list row uses ("Page 143", "12:04"). Structured so
/// the reader formats it for its own locale; [`fmt::Display`] is the English default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", content = "v", rename_all = "snake_case")]
pub enum ResumeLabel {
    /// Nothing to say: an image, or a file left at its start.
    Unlabelled,
    /// A page, counting from one.
    Page {
        /// The page number a reader sees.
        number: u32,
    },
    /// A position in a recording.
    Time {
        /// Whole seconds from the start.
        secs: u64,
    },
    /// A line, counting from one.
    Line {
        /// The line number an editor shows.
        number: u32,
    },
}

/// The label for `resume`. Total over `Resume`: a state with no position worth a row's room is
/// [`ResumeLabel::Unlabelled`].
pub fn resume_label(resume: &Resume) -> ResumeLabel {
    match resume {
        Resume::Pdf { page, .. } => ResumeLabel::Page {
            number: page.0.saturating_add(1),
        },
        Resume::Media { at, .. } => ResumeLabel::Time {
            secs: at.as_millis() / 1_000,
        },
        Resume::Text { line } => ResumeLabel::Line {
            number: line.0.saturating_add(1),
        },
        Resume::Raster { .. } | Resume::Nothing => ResumeLabel::Unlabelled,
    }
}

impl fmt::Display for ResumeLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResumeLabel::Unlabelled => Ok(()),
            ResumeLabel::Page { number } => write!(f, "Page {number}"),
            ResumeLabel::Line { number } => write!(f, "Line {number}"),
            ResumeLabel::Time { secs } => {
                let (hours, minutes, seconds) = (secs / 3600, secs / 60 % 60, secs % 60);
                if hours > 0 {
                    write!(f, "{hours}:{minutes:02}:{seconds:02}")
                } else {
                    write!(f, "{minutes}:{seconds:02}")
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::{
        DocPoint, DocUnit, LineIndex, MediaTime, PageIndex, Percent, Permille, TrackChoice, Volume,
        Zoom,
    };

    fn media(at: MediaTime) -> Resume {
        Resume::Media {
            at,
            volume: Volume::clamped(Percent(100)),
            audio: TrackChoice::Auto,
            subtitles: TrackChoice::Off,
        }
    }

    #[test]
    fn a_resume_becomes_its_label() {
        let cases: Vec<(&str, Resume, ResumeLabel)> = vec![
            (
                "pdf is one-based",
                Resume::Pdf {
                    page: PageIndex(142),
                    offset: Permille(0),
                    zoom: Zoom::Fit,
                },
                ResumeLabel::Page { number: 143 },
            ),
            (
                "pdf page saturates",
                Resume::Pdf {
                    page: PageIndex(u32::MAX),
                    offset: Permille(0),
                    zoom: Zoom::Fit,
                },
                ResumeLabel::Page { number: u32::MAX },
            ),
            (
                "media rounds down to seconds",
                media(MediaTime::from_millis(724_999)),
                ResumeLabel::Time { secs: 724 },
            ),
            (
                "text is one-based",
                Resume::Text { line: LineIndex(0) },
                ResumeLabel::Line { number: 1 },
            ),
            (
                "raster has none",
                Resume::Raster {
                    zoom: Zoom::Fit,
                    centre: DocPoint {
                        x: DocUnit(0),
                        y: DocUnit(0),
                    },
                },
                ResumeLabel::Unlabelled,
            ),
            ("nothing", Resume::Nothing, ResumeLabel::Unlabelled),
        ];
        for (name, resume, want) in cases {
            assert_eq!(resume_label(&resume), want, "{name}");
        }
    }

    #[test]
    fn labels_read_as_a_person_says_them() {
        const CASES: &[(&str, ResumeLabel, &str)] = &[
            ("page", ResumeLabel::Page { number: 143 }, "Page 143"),
            ("line", ResumeLabel::Line { number: 481 }, "Line 481"),
            ("minutes", ResumeLabel::Time { secs: 724 }, "12:04"),
            ("under a minute", ResumeLabel::Time { secs: 5 }, "0:05"),
            ("hours", ResumeLabel::Time { secs: 3723 }, "1:02:03"),
            ("none", ResumeLabel::Unlabelled, ""),
        ];
        for (name, label, want) in CASES {
            assert_eq!(&label.to_string(), want, "{name}");
        }
    }

    #[test]
    fn every_label_round_trips_in_its_stored_form() {
        const CASES: &[(&str, ResumeLabel, &str)] = &[
            ("none", ResumeLabel::Unlabelled, r#"{"kind":"unlabelled"}"#),
            (
                "page",
                ResumeLabel::Page { number: 3 },
                r#"{"kind":"page","v":{"number":3}}"#,
            ),
            (
                "time",
                ResumeLabel::Time { secs: 9 },
                r#"{"kind":"time","v":{"secs":9}}"#,
            ),
            (
                "line",
                ResumeLabel::Line { number: 8 },
                r#"{"kind":"line","v":{"number":8}}"#,
            ),
        ];
        for (name, label, json) in CASES {
            assert_eq!(
                &serde_json::to_string(label).unwrap(),
                json,
                "{name} writes"
            );
            assert_eq!(
                &serde_json::from_str::<ResumeLabel>(json).unwrap(),
                label,
                "{name} reads"
            );
        }
    }
}
