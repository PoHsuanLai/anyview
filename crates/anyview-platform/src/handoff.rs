//! A [`Handoff`] on the wire: `Handoff(s file, s resume, t results, as entries)`. The place the
//! pane held travels as the JSON a `Resume` is stored as, and the results as their paths with
//! the id of the search they came from; none means the file came from no list. Each side parses
//! what it receives: a relative path, a place that is not a `Resume` or a file missing from its
//! own results is refused by name.

use crate::instance::Handoff;
use anyview_core::{FilePath, NonEmpty, ResultsId, Resume, Sequence, SequenceOrigin};

/// What a handoff is made of on the bus, in the order of the method's arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Wire {
    pub(crate) file: String,
    pub(crate) resume: String,
    pub(crate) results: u64,
    pub(crate) entries: Vec<String>,
}

/// `handoff` as the method's arguments. Writing a `Resume` cannot fail: its fields are integers
/// and words.
pub(crate) fn encode(handoff: &Handoff) -> Wire {
    let text = |file: &FilePath| file.as_path().to_string_lossy().into_owned();
    let (results, entries) = match &handoff.sequence {
        Some(sequence) => (
            match sequence.origin() {
                SequenceOrigin::Results(id) => id.0,
                SequenceOrigin::Folder(_) | SequenceOrigin::Selection => 0,
            },
            sequence.entries().iter().map(text).collect(),
        ),
        None => (0, Vec::new()),
    };
    Wire {
        file: text(&handoff.file),
        resume: serde_json::to_string(&handoff.resume).unwrap_or_default(),
        results,
        entries,
    }
}

/// The handoff `wire` says, or why it is not one.
pub(crate) fn decode(wire: &Wire) -> Result<Handoff, String> {
    let path = |text: &String| FilePath::new(text).map_err(|error| error.to_string());
    let file = path(&wire.file)?;
    let resume: Resume = serde_json::from_str(&wire.resume)
        .map_err(|error| format!("the place is not a resume: {error}"))?;
    let entries = wire
        .entries
        .iter()
        .map(path)
        .collect::<Result<Vec<_>, _>>()?;
    let sequence = match NonEmpty::from_vec(entries) {
        None => None,
        Some(entries) => Some(
            Sequence::starting_at(
                entries,
                &file,
                SequenceOrigin::Results(ResultsId(wire.results)),
            )
            .map_err(|error| error.to_string())?,
        ),
    };
    Ok(Handoff {
        file,
        resume,
        sequence,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::{PageIndex, Permille, Zoom};

    fn file(text: &str) -> FilePath {
        FilePath::new(text).unwrap()
    }

    fn results(at: &str, all: &[&str]) -> Sequence {
        let entries = NonEmpty::from_vec(all.iter().map(|text| file(text)).collect()).unwrap();
        Sequence::starting_at(entries, &file(at), SequenceOrigin::Results(ResultsId(7))).unwrap()
    }

    #[test]
    fn a_handoff_survives_the_wire() {
        let page = Resume::Pdf {
            page: PageIndex(4),
            offset: Permille(250),
            zoom: Zoom::Fit,
        };
        let cases = [
            Handoff {
                file: file("/docs/b.pdf"),
                resume: page,
                sequence: Some(results(
                    "/docs/b.pdf",
                    &["/docs/a.pdf", "/docs/b.pdf", "/c d.png"],
                )),
            },
            Handoff {
                file: file("/docs/a.pdf"),
                resume: Resume::Nothing,
                sequence: None,
            },
        ];
        for handoff in cases {
            let wire = encode(&handoff);
            assert_eq!(decode(&wire), Ok(handoff.clone()), "{handoff:?}");
        }
    }

    #[test]
    fn the_wire_form_names_the_file_the_place_and_the_search() {
        let handoff = Handoff {
            file: file("/docs/b.pdf"),
            resume: Resume::Nothing,
            sequence: Some(results("/docs/b.pdf", &["/docs/a.pdf", "/docs/b.pdf"])),
        };
        assert_eq!(
            encode(&handoff),
            Wire {
                file: "/docs/b.pdf".to_owned(),
                resume: r#"{"kind":"nothing"}"#.to_owned(),
                results: 7,
                entries: vec!["/docs/a.pdf".to_owned(), "/docs/b.pdf".to_owned()],
            }
        );
    }

    #[test]
    fn what_cannot_be_a_handoff_is_refused() {
        let good = Wire {
            file: "/a.png".to_owned(),
            resume: r#"{"kind":"nothing"}"#.to_owned(),
            results: 1,
            entries: vec!["/a.png".to_owned(), "/b.png".to_owned()],
        };
        assert!(decode(&good).is_ok());
        let cases = [
            (
                "a relative file",
                Wire {
                    file: "a.png".to_owned(),
                    ..good.clone()
                },
            ),
            (
                "a place that is not a resume",
                Wire {
                    resume: "page 3".to_owned(),
                    ..good.clone()
                },
            ),
            (
                "a relative result",
                Wire {
                    entries: vec!["/a.png".to_owned(), "b.png".to_owned()],
                    ..good.clone()
                },
            ),
            (
                "a file that is not among its results",
                Wire {
                    entries: vec!["/b.png".to_owned()],
                    ..good.clone()
                },
            ),
        ];
        for (name, wire) in cases {
            assert!(decode(&wire).is_err(), "{name}");
        }
    }
}
