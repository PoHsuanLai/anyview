//! Choosing a file through the desktop portal's `FileChooser.OpenFile`: the desktop's own dialog,
//! the one a sandboxed program must use, answers the `file://` URIs of what was chosen on a
//! Request object (`portal`).

use super::portal::{DESKTOP, DESKTOP_PATH, Response, UNPARENTED, answers, failure};
use crate::env::Env;
use crate::error::PlatformError;
use crate::picker::{FileKinds, PickOutcome, Picker};
use anyview_core::FilePath;
use percent_encoding::percent_decode_str;
use std::collections::HashMap;
use zbus::Proxy;
use zbus::zvariant::{Array, Structure, Type, Value};

const FILE_CHOOSER: &str = "org.freedesktop.portal.FileChooser";
const CALL: &str = "choose a file through the portal";
const TITLE: &str = "Open";

/// [`Picker`] through `org.freedesktop.portal.FileChooser`.
#[derive(Debug, Clone)]
pub struct PortalPicker {
    env: Env,
}

impl PortalPicker {
    /// A file dialog on `env`'s session bus.
    pub fn new(env: Env) -> Self {
        PortalPicker { env }
    }
}

impl Picker for PortalPicker {
    async fn pick(&self, kinds: &FileKinds) -> Result<PickOutcome, PlatformError> {
        let Ok(builder) = self.env.session_builder() else {
            return Ok(PickOutcome::NoDialog);
        };
        let Ok(connection) = builder.build().await else {
            return Ok(PickOutcome::NoDialog);
        };
        let mut answered = answers(&connection, "choose", CALL).await?;
        let mut options: HashMap<&str, Value<'_>> = HashMap::from([
            ("handle_token", Value::from("choose")),
            ("modal", Value::from(true)),
            ("multiple", Value::from(true)),
        ]);
        if let Some((filters, current)) = filters_of(kinds) {
            options.insert("filters", filters);
            options.insert("current_filter", current);
        }
        let Ok(proxy) = Proxy::new(&connection, DESKTOP, DESKTOP_PATH, FILE_CHOOSER).await else {
            return Ok(PickOutcome::NoDialog);
        };
        // No portal that offers the chooser is the same as no dialog; once it is up, a failure is
        // the call's.
        if proxy
            .call_method("OpenFile", &(UNPARENTED, TITLE, options))
            .await
            .is_err()
        {
            return Ok(PickOutcome::NoDialog);
        }
        let (response, results) = answered.next_answer().await?;
        match response {
            Response::Success => {
                let uris: Vec<String> = results
                    .get("uris")
                    .and_then(|value| <Vec<String>>::try_from(value.try_clone().ok()?).ok())
                    .ok_or_else(|| failure(CALL, "no files in the answer"))?;
                let files: Vec<FilePath> = uris.iter().filter_map(|uri| path_of(uri)).collect();
                if files.is_empty() {
                    Err(failure(CALL, "the answer names no local file"))
                } else {
                    Ok(PickOutcome::Chosen(files))
                }
            }
            Response::Cancelled => Ok(PickOutcome::Cancelled),
            Response::Ended => Err(failure(CALL, "the dialog ended without an answer")),
        }
    }
}

/// What the first filter is called.
const SUPPORTED: &str = "Supported files";
/// What the filter that lists everything is called.
const EVERYTHING: &str = "All files";
/// A filter item that is a file name pattern.
const GLOB: u32 = 0;
/// A filter item that is a media type.
const MIME: u32 = 1;

/// One filter as the portal takes it: `(sa(us))`, a label and the items that match.
fn filter(label: &str, items: &[(u32, String)]) -> Option<Value<'static>> {
    let mut inner = Array::new(<(u32, String)>::SIGNATURE);
    for (kind, text) in items {
        inner
            .append(Value::from(Structure::from((*kind, text.clone()))))
            .ok()?;
    }
    Some(Value::from(Structure::from((label.to_owned(), inner))))
}

/// The portal's `filters` (`a(sa(us))`: "Supported files", then "All files") and its
/// `current_filter` (the first of them), or `None` when there is nothing to filter by. The portal
/// hides the files a filter does not match; it has no way to grey them out.
fn filters_of(kinds: &FileKinds) -> Option<(Value<'static>, Value<'static>)> {
    if kinds.is_empty() {
        return None;
    }
    let supported: Vec<(u32, String)> = kinds
        .mimes
        .iter()
        .map(|mime| (MIME, mime.clone()))
        .chain(kinds.globs.iter().map(|glob| (GLOB, glob.clone())))
        .collect();
    let first = filter(SUPPORTED, &supported)?;
    let everything = filter(EVERYTHING, &[(GLOB, "*".to_owned())])?;
    let mut list = Array::new(<(String, Vec<(u32, String)>)>::SIGNATURE);
    list.append(first.try_clone().ok()?).ok()?;
    list.append(everything).ok()?;
    Some((Value::from(list), first))
}

/// The local file a `file://` URI names, with its percent-escapes decoded as bytes (a path need
/// not be UTF-8). Any other scheme, a host that is not this machine and a relative path are none.
fn path_of(uri: &str) -> Option<FilePath> {
    use std::os::unix::ffi::OsStrExt;
    let rest = uri.strip_prefix("file://")?;
    let path = rest.strip_prefix("localhost").unwrap_or(rest);
    if !path.starts_with('/') {
        return None;
    }
    let bytes: Vec<u8> = percent_decode_str(path).collect();
    FilePath::new(std::ffi::OsStr::from_bytes(&bytes)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chooser_starts_on_supported_files_and_keeps_all_files_beside_them() {
        use zbus::zvariant::Signature;
        let kinds = FileKinds {
            mimes: vec!["image/png".to_owned(), "application/pdf".to_owned()],
            globs: vec!["*.jsonl".to_owned()],
        };
        let (filters, current) = filters_of(&kinds).expect("filters");
        assert_eq!(
            filters.value_signature(),
            &Signature::try_from("a(sa(us))").unwrap()
        );
        assert_eq!(
            current.value_signature(),
            &Signature::try_from("(sa(us))").unwrap()
        );
        let shown = format!("{filters}");
        for part in [
            "Supported files",
            "All files",
            "image/png",
            "application/pdf",
            "*.jsonl",
        ] {
            assert!(shown.contains(part), "{part} in {shown}");
        }
        assert!(
            shown.find("Supported files") < shown.find("All files"),
            "Supported files is first: {shown}"
        );
        assert_eq!(format!("{current}").matches("image/png").count(), 1);
        assert!(!format!("{current}").contains("All files"));
        assert!(
            filters_of(&FileKinds::default()).is_none(),
            "nothing to filter by"
        );
    }

    #[test]
    fn a_file_uri_names_the_local_file_it_escapes() {
        // name, uri, the path it names
        const CASES: &[(&str, &str, Option<&str>)] = &[
            ("plain", "file:///home/a/b.png", Some("/home/a/b.png")),
            (
                "escaped",
                "file:///home/a%20b/%C3%A9.png",
                Some("/home/a b/é.png"),
            ),
            ("localhost", "file://localhost/tmp/x", Some("/tmp/x")),
            ("another host", "file://other/tmp/x", None),
            ("another scheme", "https://example.org/x", None),
            ("not a uri", "/tmp/x", None),
        ];
        for (name, uri, want) in CASES {
            let got = path_of(uri);
            assert_eq!(
                got.as_ref()
                    .map(|path| path.as_path().to_string_lossy().into_owned()),
                want.map(str::to_owned),
                "{name}"
            );
        }
    }
}
