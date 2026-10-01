//! Which link targets the rendered document may carry. The frame never navigates by itself, but a
//! `javascript:` target must not survive into markup other code may one day act on.

/// The schemes a link may use.
const SCHEMES: &[&str] = &["http", "https", "mailto"];

/// The scheme of `url` (`https` in `https://x`), lower-cased, when it has one: the letters, digits,
/// `+`, `-` and `.` before the first `:`, starting with a letter. `a/b:c` has none, since a `/`
/// ends the scheme part.
pub(super) fn scheme(url: &str) -> Option<String> {
    let (head, _) = url.split_once(':')?;
    let mut chars = head.chars();
    let valid = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
    valid.then(|| head.to_ascii_lowercase())
}

/// `url` when it is safe to put in a link: a fragment, a relative reference, or one of the
/// allowed schemes. Anything else becomes `#`.
pub(super) fn safe_link(url: &str) -> String {
    match scheme(url.trim()) {
        Some(scheme) if !SCHEMES.contains(&scheme.as_str()) => "#".to_owned(),
        Some(_) | None => url.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_targets_keep_safe_schemes_and_lose_the_rest() {
        const CASES: &[(&str, &str, &str)] = &[
            (
                "https",
                "https://example.com/a?b=c",
                "https://example.com/a?b=c",
            ),
            ("http", "http://example.com", "http://example.com"),
            ("mail", "mailto:a@b.c", "mailto:a@b.c"),
            ("fragment", "#install", "#install"),
            ("relative file", "docs/guide.md", "docs/guide.md"),
            ("relative with a colon after a slash", "a/b:c", "a/b:c"),
            ("javascript", "javascript:alert(1)", "#"),
            ("javascript in capitals", "JaVaScRiPt:alert(1)", "#"),
            ("padded javascript", "  javascript:alert(1)", "#"),
            ("data", "data:text/html,<b>x</b>", "#"),
            ("file", "file:///etc/passwd", "#"),
            ("scheme only", "vbscript:x", "#"),
        ];
        for (name, url, want) in CASES {
            assert_eq!(safe_link(url), *want, "{name}");
        }
    }

    #[test]
    fn schemes_are_read_at_their_grammar() {
        const CASES: &[(&str, Option<&str>)] = &[
            ("https://x", Some("https")),
            ("HTTPS://x", Some("https")),
            ("a+b-c.d:x", Some("a+b-c.d")),
            ("1http://x", None),
            ("//host/x", None),
            ("no colon", None),
            (":x", None),
            ("a b:c", None),
        ];
        for (url, want) in CASES {
            assert_eq!(scheme(url).as_deref(), *want, "{url}");
        }
    }
}
