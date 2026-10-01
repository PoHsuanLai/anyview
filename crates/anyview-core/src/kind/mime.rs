//! `Mime`: a media type, parsed.

use crate::error::CoreError;
use std::borrow::Cow;

/// A media type as `type/subtype`, lower-case, with no parameters (`text/plain`, never
/// `text/plain; charset=utf-8`). Parsed once: holding one proves the text is well formed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Mime(Cow<'static, str>);

impl Mime {
    /// The type of bytes that nothing else describes.
    pub const OCTET_STREAM: Mime = Mime(Cow::Borrowed("application/octet-stream"));

    /// `text` as a media type, lower-cased, or why it is not one.
    pub fn parse(text: &str) -> Result<Self, CoreError> {
        let lower = text.to_ascii_lowercase();
        let token = |part: &str| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "!#$&^_.+-".contains(c))
        };
        match lower.split_once('/') {
            Some((kind, subtype)) if token(kind) && token(subtype) => Ok(Mime(Cow::Owned(lower))),
            Some(_) | None => Err(CoreError::MimeInvalid {
                text: text.to_owned(),
            }),
        }
    }

    /// A type from this crate's own tables, which a test checks parse.
    pub(crate) const fn known(text: &'static str) -> Self {
        Mime(Cow::Borrowed(text))
    }

    /// The `type/subtype` text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Mime {
    type Error = CoreError;

    fn try_from(text: String) -> Result<Self, CoreError> {
        Mime::parse(&text)
    }
}

impl From<Mime> for String {
    fn from(mime: Mime) -> String {
        mime.0.into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_types_parse_to_their_lower_case_essence() {
        const CASES: &[(&str, &str, Option<&str>)] = &[
            ("plain", "text/plain", Some("text/plain")),
            ("upper case", "Image/PNG", Some("image/png")),
            (
                "suffix",
                "application/epub+zip",
                Some("application/epub+zip"),
            ),
            (
                "vendor",
                "application/vnd.ms-excel",
                Some("application/vnd.ms-excel"),
            ),
            ("parameter", "text/plain; charset=utf-8", None),
            ("no slash", "text", None),
            ("empty type", "/png", None),
            ("empty subtype", "image/", None),
            ("two slashes", "a/b/c", None),
            ("space", "text /plain", None),
            ("empty", "", None),
        ];
        for (name, text, want) in CASES {
            assert_eq!(
                Mime::parse(text).ok().as_ref().map(Mime::as_str),
                *want,
                "{name}"
            );
        }
    }

    #[test]
    fn a_media_type_round_trips_and_a_bad_one_is_refused_on_load() {
        let mime = Mime::parse("image/webp").unwrap();
        let json = serde_json::to_string(&mime).unwrap();
        assert_eq!(json, "\"image/webp\"");
        assert_eq!(serde_json::from_str::<Mime>(&json).unwrap(), mime);
        assert!(serde_json::from_str::<Mime>("\"nonsense\"").is_err());
        assert_eq!(
            Mime::parse("application/octet-stream"),
            Ok(Mime::OCTET_STREAM)
        );
    }
}
