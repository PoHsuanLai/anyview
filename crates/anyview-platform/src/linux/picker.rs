//! Choosing a file through the desktop portal's `FileChooser.OpenFile`: the desktop's own dialog,
//! the one a sandboxed program must use, answers the `file://` URIs of what was chosen on a
//! Request object (`portal`).

use super::portal::{DESKTOP, DESKTOP_PATH, Response, UNPARENTED, answers, failure};
use crate::env::Env;
use crate::error::PlatformError;
use crate::picker::{PickOutcome, Picker};
use anyview_core::FilePath;
use percent_encoding::percent_decode_str;
use std::collections::HashMap;
use zbus::Proxy;
use zbus::zvariant::Value;

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
    async fn pick(&self) -> Result<PickOutcome, PlatformError> {
        let Ok(builder) = self.env.session_builder() else {
            return Ok(PickOutcome::NoDialog);
        };
        let Ok(connection) = builder.build().await else {
            return Ok(PickOutcome::NoDialog);
        };
        let mut answered = answers(&connection, "choose", CALL).await?;
        let options: HashMap<&str, Value<'_>> = HashMap::from([
            ("handle_token", Value::from("choose")),
            ("modal", Value::from(true)),
            ("multiple", Value::from(true)),
        ]);
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
