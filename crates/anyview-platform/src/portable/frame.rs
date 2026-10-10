//! A [`Request`] on the per-user socket: one line of JSON, answered by one line of JSON.
//!
//! `{"op":"open","files":[...]}`, `{"op":"peek","file":...}`, `{"op":"play","file":...}` and
//! `{"op":"handoff", ...}` (the same fields a D-Bus `Handoff` carries, from `handoff`). The
//! answer is `{"ok":true}`, or `{"ok":false,"why":...}` when the running viewer refused the
//! request. The receiving side parses everything it is sent, as the D-Bus service does: a
//! relative path or a place that is not a `Resume` is refused by name.

use crate::handoff::{self, HandoffError, Wire};
use crate::instance::Request;
use anyview_core::{CoreError, FilePath};
use serde_json::{Value, json};

/// Why a line is not a request, an answer is not an answer, or a request was not taken. The
/// viewer's side writes it as the `why` of a refusal; the client's side reads it back.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(super) enum FrameError {
    /// The line is not JSON.
    #[error("not a request: {0}")]
    NotJson(String),
    /// A text field is missing or is not text.
    #[error("the request has no text field {0:?}")]
    NoText(&'static str),
    /// A list field is missing or is not a list.
    #[error("the request has no list field {0:?}")]
    NoList(&'static str),
    /// A list holds an item that is not text.
    #[error("{0:?} holds something that is not text")]
    ItemNotText(&'static str),
    /// A number field is missing or is not a number.
    #[error("the request has no number field {0:?}")]
    NoNumber(&'static str),
    /// The `op` names nothing the viewer does.
    #[error("unknown request {0:?}")]
    UnknownOp(String),
    /// A path is relative.
    #[error("{0}")]
    Path(#[from] CoreError),
    /// A handoff is not one.
    #[error("{0}")]
    Handoff(#[from] HandoffError),
    /// The request was longer than [`MAX_LINE`] or the connection ended before its newline.
    #[error("the request was cut short or too long")]
    CutShort,
    /// The request could not be read from the connection.
    #[error("the request could not be read: {0}")]
    Unreadable(String),
    /// The viewer is shutting down and takes no more requests.
    #[error("the viewer is closing")]
    Closing,
    /// The running viewer refused the request, for the reason it gave.
    #[error("{0}")]
    Refused(String),
    /// The viewer's answer is not JSON.
    #[error("an answer that is not JSON: {0}")]
    AnswerNotJson(String),
    /// The viewer's answer says neither yes nor no.
    #[error("an answer that says neither yes nor no")]
    AnswerUndecided,
}

/// The most a request may weigh. A handoff names a search's results, so it can be long, but no
/// honest request is this large; anything past it is cut off and refused.
pub(super) const MAX_LINE: u64 = 16 * 1024 * 1024;

/// `request` as the line a client sends, without its newline.
pub(super) fn encode(request: &Request) -> String {
    let text = |file: &FilePath| file.as_path().to_string_lossy().into_owned();
    match request {
        Request::Open(files) => {
            json!({ "op": "open", "files": files.iter().map(text).collect::<Vec<_>>() })
        }
        Request::Peek(file) => json!({ "op": "peek", "file": text(file) }),
        Request::Play(file) => json!({ "op": "play", "file": text(file) }),
        Request::Handoff(handed) => {
            let wire = handoff::encode(handed);
            json!({
                "op": "handoff",
                "file": wire.file,
                "resume": wire.resume,
                "results": wire.results,
                "entries": wire.entries,
            })
        }
    }
    .to_string()
}

/// The request `line` says, or why it is not one.
pub(super) fn decode(line: &str) -> Result<Request, FrameError> {
    let value: Value =
        serde_json::from_str(line).map_err(|error| FrameError::NotJson(error.to_string()))?;
    let field = |name: &str| value.get(name);
    let string = |name: &'static str| {
        field(name)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or(FrameError::NoText(name))
    };
    let strings = |name: &'static str| {
        field(name)
            .and_then(Value::as_array)
            .ok_or(FrameError::NoList(name))?
            .iter()
            .map(|item| {
                item.as_str()
                    .map(str::to_owned)
                    .ok_or(FrameError::ItemNotText(name))
            })
            .collect::<Result<Vec<_>, _>>()
    };
    match string("op")?.as_str() {
        "open" => Ok(Request::Open(
            strings("files")?
                .iter()
                .map(FilePath::new)
                .collect::<Result<Vec<_>, _>>()?,
        )),
        "peek" => Ok(Request::Peek(FilePath::new(string("file")?)?)),
        "play" => Ok(Request::Play(FilePath::new(string("file")?)?)),
        "handoff" => {
            let wire = Wire {
                file: string("file")?,
                resume: string("resume")?,
                results: field("results")
                    .and_then(Value::as_u64)
                    .ok_or(FrameError::NoNumber("results"))?,
                entries: strings("entries")?,
            };
            Ok(Request::Handoff(handoff::decode(&wire)?))
        }
        other => Err(FrameError::UnknownOp(other.to_owned())),
    }
}

/// The line the viewer answers with.
pub(super) fn answer(result: &Result<(), FrameError>) -> String {
    match result {
        Ok(()) => json!({ "ok": true }),
        Err(why) => json!({ "ok": false, "why": why.to_string() }),
    }
    .to_string()
}

/// What an answer line says: nothing when the request was taken, the viewer's reason when not.
pub(super) fn understand(line: &str) -> Result<(), FrameError> {
    let value: Value =
        serde_json::from_str(line).map_err(|error| FrameError::AnswerNotJson(error.to_string()))?;
    match value.get("ok").and_then(Value::as_bool) {
        Some(true) => Ok(()),
        Some(false) => Err(FrameError::Refused(
            value
                .get("why")
                .and_then(Value::as_str)
                .unwrap_or("no reason given")
                .to_owned(),
        )),
        None => Err(FrameError::AnswerUndecided),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::instance::Handoff;
    use anyview_core::{NonEmpty, ResultsId, Resume, Sequence, SequenceOrigin};

    fn file(text: &str) -> FilePath {
        FilePath::new(text).unwrap()
    }

    #[test]
    fn every_request_survives_the_wire() {
        let a = file("/pics/a.png");
        let b = file("/pics/b b.png");
        let entries = NonEmpty::from_vec(vec![a.clone(), b.clone()]).unwrap();
        let sequence =
            Sequence::starting_at(entries, &b, SequenceOrigin::Results(ResultsId(7))).unwrap();
        let cases = [
            Request::Open(Vec::new()),
            Request::Open(vec![a.clone(), b.clone()]),
            Request::Peek(a.clone()),
            Request::Play(b.clone()),
            Request::Handoff(Handoff {
                file: b,
                resume: Resume::Nothing,
                sequence: Some(sequence),
            }),
            Request::Handoff(Handoff {
                file: a,
                resume: Resume::Nothing,
                sequence: None,
            }),
        ];
        for request in cases {
            assert_eq!(
                decode(&encode(&request)),
                Ok(request.clone()),
                "{request:?}"
            );
        }
    }

    #[test]
    fn what_is_not_a_request_is_refused_by_name() {
        const CASES: &[(&str, &str, &str)] = &[
            ("not json", "hello", "not a request"),
            ("no op", r#"{"file":"/a"}"#, "\"op\""),
            ("unknown op", r#"{"op":"quit"}"#, "unknown request"),
            ("files not text", r#"{"op":"open","files":[1]}"#, "not text"),
            ("no files list", r#"{"op":"open"}"#, "\"files\""),
        ];
        for (name, line, mentions) in CASES {
            let refused = decode(line).unwrap_err().to_string();
            assert!(refused.contains(mentions), "{name}: {refused}");
        }
        assert!(decode(r#"{"op":"peek","file":"a/b.png"}"#).is_err());
    }

    #[test]
    fn an_answer_says_yes_or_the_reason() {
        assert_eq!(understand(&answer(&Ok(()))), Ok(()));
        assert_eq!(
            understand(&answer(&Err(FrameError::Closing))),
            Err(FrameError::Refused(FrameError::Closing.to_string()))
        );
        assert_eq!(understand("{}"), Err(FrameError::AnswerUndecided));
    }
}
