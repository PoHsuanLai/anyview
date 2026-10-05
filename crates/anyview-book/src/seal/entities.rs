//! The character references an attribute value may hold, decoded for reading and escaped for
//! writing.

/// `value` with the references an attribute commonly holds decoded: the five named ones,
/// `&nbsp;` and numbers. Anything else stays as written.
pub(super) fn decode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        match reference(rest) {
            Some((c, length)) => {
                out.push(c);
                rest = &rest[length..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// The character the reference at the start of `text` (which begins with `&`) stands for and the
/// length of the reference.
fn reference(text: &str) -> Option<(char, usize)> {
    let end = text.find(';').filter(|end| *end <= 10)?;
    let name = &text[1..end];
    let c = match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => '\u{a0}',
        _ => {
            let number = name.strip_prefix('#')?;
            let code = match number.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => number.parse().ok()?,
            };
            char::from_u32(code)?
        }
    };
    Some((c, end + 1))
}

/// `value` written inside double quotes.
pub(super) fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_decode_and_text_escapes() {
        assert_eq!(decode("a&amp;b &#65;&#x42; &bogus; &"), "a&b AB &bogus; &");
        assert_eq!(escape("a&b\"<>"), "a&amp;b&quot;&lt;&gt;");
    }
}
