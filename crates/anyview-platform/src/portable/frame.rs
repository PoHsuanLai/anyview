//! A [`Request`] on the per-user socket: one line of JSON, answered by one line of JSON.
//!
//! `{"op":"open","files":[...]}`, `{"op":"peek","file":...}`, `{"op":"play","file":...}` and
//! `{"op":"handoff", ...}` (the same fields a D-Bus `Handoff` carries, from `handoff`). The
//! answer is `{"ok":true}`, or `{"ok":false,"why":...}` when the running viewer refused the
//! request. The receiving side parses everything it is sent, as the D-Bus service does: a
//! relative path or a place that is not a `Resume` is refused by name.

use crate::handoff::{self, Wire};
use crate::instance::Request;
use anyview_core::FilePath;
use serde_json::{Value, json};

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
pub(super) fn decode(line: &str) -> Result<Request, String> {
    let value: Value =
        serde_json::from_str(line).map_err(|error| format!("not a request: {error}"))?;
    let field = |name: &str| value.get(name);
    let string = |name: &str| {
        field(name)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("the request has no text field {name:?}"))
    };
    let strings = |name: &str| {
        field(name)
            .and_then(Value::as_array)
            .ok_or_else(|| format!("the request has no list field {name:?}"))?
            .iter()
            .map(|item| {
                item.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| format!("{name:?} holds something that is not text"))
            })
            .collect::<Result<Vec<_>, _>>()
    };
    let path = |text: &str| FilePath::new(text).map_err(|error| error.to_string());
    match string("op")?.as_str() {
        "open" => strings("files")?
            .iter()
            .map(|file| path(file))
            .collect::<Result<Vec<_>, _>>()
            .map(Request::Open),
        "peek" => path(&string("file")?).map(Request::Peek),
        "play" => path(&string("file")?).map(Request::Play),
        "handoff" => {
            let wire = Wire {
                file: string("file")?,
                resume: string("resume")?,
                results: field("results")
                    .and_then(Value::as_u64)
                    .ok_or("the request has no number field \"results\"")?,
                entries: strings("entries")?,
            };
            handoff::decode(&wire).map(Request::Handoff)
        }
        other => Err(format!("unknown request {other:?}")),
    }
}

/// The line the viewer answers with.
pub(super) fn answer(result: &Result<(), String>) -> String {
    match result {
        Ok(()) => json!({ "ok": true }),
        Err(why) => json!({ "ok": false, "why": why }),
    }
    .to_string()
}

/// What an answer line says: nothing when the request was taken, the viewer's reason when not.
pub(super) fn understand(line: &str) -> Result<(), String> {
    let value: Value = serde_json::from_str(line)
        .map_err(|error| format!("an answer that is not JSON: {error}"))?;
    match value.get("ok").and_then(Value::as_bool) {
        Some(true) => Ok(()),
        Some(false) => Err(value
            .get("why")
            .and_then(Value::as_str)
            .unwrap_or("no reason given")
            .to_owned()),
        None => Err("an answer that says neither yes nor no".to_owned()),
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
            let refused = decode(line).unwrap_err();
            assert!(refused.contains(mentions), "{name}: {refused}");
        }
        assert!(decode(r#"{"op":"peek","file":"a/b.png"}"#).is_err());
    }

    #[test]
    fn an_answer_says_yes_or_the_reason() {
        assert_eq!(understand(&answer(&Ok(()))), Ok(()));
        assert_eq!(
            understand(&answer(&Err("closing".to_owned()))),
            Err("closing".to_owned())
        );
        assert!(understand("{}").is_err());
    }
}
