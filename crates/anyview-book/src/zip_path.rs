//! Paths inside a zip: the references a package makes, resolved to entry names.

/// `%20` and its kind decoded; bytes that are not valid escapes stay as they are.
pub(crate) fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let escaped = (bytes[i] == b'%')
            .then(|| bytes.get(i + 1..i + 3))
            .flatten()
            .and_then(|hex| std::str::from_utf8(hex).ok())
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match escaped {
            Some(byte) => {
                out.push(byte);
                i += 3;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The directory part of the entry name `path`, with its trailing slash, or empty.
pub(crate) fn directory_of(path: &str) -> &str {
    path.rfind('/').map_or("", |at| &path[..=at])
}

/// Whether `reference` names something outside the zip: it has a scheme, or starts `//`.
fn is_remote(reference: &str) -> bool {
    if reference.starts_with("//") {
        return true;
    }
    let Some((head, _)) = reference.split_once(':') else {
        return false;
    };
    let mut chars = head.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

/// The entry name `reference` points at from a document in `directory` (with its trailing slash),
/// without query or fragment, percent-decoded and with `.` and `..` folded away. `None` for a
/// remote reference, an empty one, or one that climbs out of the zip.
pub(crate) fn resolve(directory: &str, reference: &str) -> Option<String> {
    let reference = reference.trim();
    if is_remote(reference) {
        return None;
    }
    let end = reference.find(['?', '#']).unwrap_or(reference.len());
    let decoded = percent_decode(&reference[..end]);
    if decoded.is_empty() {
        return None;
    }
    let joined = match decoded.strip_prefix('/') {
        Some(rooted) => rooted.to_owned(),
        None => format!("{directory}{decoded}"),
    };
    let mut parts: Vec<&str> = Vec::new();
    for part in joined.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            name => parts.push(name),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_resolve_against_the_directory_of_the_document() {
        // name, directory, reference, entry
        const CASES: &[(&str, &str, &str, Option<&str>)] = &[
            ("sibling", "OEBPS/", "ch1.xhtml", Some("OEBPS/ch1.xhtml")),
            (
                "up one",
                "OEBPS/text/",
                "../img/a.png",
                Some("OEBPS/img/a.png"),
            ),
            (
                "fragment dropped",
                "OEBPS/",
                "ch1.xhtml#s2",
                Some("OEBPS/ch1.xhtml"),
            ),
            ("escapes decoded", "", "my%20file.png", Some("my file.png")),
            (
                "rooted",
                "OEBPS/",
                "/META-INF/x.xml",
                Some("META-INF/x.xml"),
            ),
            ("remote", "", "https://example.com/a.png", None),
            ("scheme relative", "", "//example.com/a.png", None),
            ("data", "", "data:image/png;base64,AA", None),
            ("climbs out", "a/", "../../x", None),
            ("fragment only", "a/", "#top", None),
        ];
        for (name, directory, reference, want) in CASES {
            assert_eq!(resolve(directory, reference).as_deref(), *want, "{name}");
        }
    }
}
