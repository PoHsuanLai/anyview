//! CSS escapes in names, decoded before the sealer looks for what it must remove: `u\72l(` is
//! `url(` to a browser, and `@\69mport` is `@import`.
//!
//! Only an escape that stands for a letter, digit, hyphen or underscore is decoded. Every other
//! escape (a quote, a parenthesis, a backslash, a newline, a control character) stays as written,
//! so decoding never changes where a string or a function ends.

/// `css` with each escape for a name character replaced by the character.
pub(super) fn unescaped(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(slash) = rest.find('\\') {
        out.push_str(&rest[..slash]);
        let escape = &rest[slash + 1..];
        let (decoded, used) = one(escape);
        match decoded {
            Some(c) => out.push(c),
            None => out.push_str(&rest[slash..slash + 1 + used]),
        }
        rest = &escape[used..];
    }
    out.push_str(rest);
    out
}

/// The name character the escape at the start of `after` (the text after the backslash) stands
/// for, and how many bytes of `after` it takes, a hex escape's one closing space included. A
/// `None` character means the escape is left as written, for the bytes counted.
fn one(after: &str) -> (Option<char>, usize) {
    let digits = after
        .bytes()
        .take(6)
        .take_while(u8::is_ascii_hexdigit)
        .count();
    if digits > 0 {
        let code = u32::from_str_radix(&after[..digits], 16).unwrap_or(0);
        return match char::from_u32(code).filter(is_name_char) {
            Some(c) => (Some(c), digits + closing_space(&after[digits..])),
            None => (None, digits),
        };
    }
    match after.chars().next() {
        Some(c) if is_name_char(&c) => (Some(c), c.len_utf8()),
        Some(c) => (None, c.len_utf8()),
        None => (None, 0),
    }
}

/// The bytes of the one whitespace that ends a hex escape: a space, tab, line feed, form feed or
/// a carriage return with its line feed.
fn closing_space(after: &str) -> usize {
    if after.starts_with("\r\n") {
        2
    } else if after.starts_with([' ', '\t', '\n', '\r', '\u{c}']) {
        1
    } else {
        0
    }
}

fn is_name_char(c: &char) -> bool {
    c.is_ascii_alphanumeric() || *c == '-' || *c == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_for_name_characters_are_decoded_and_all_others_are_left() {
        // name, css, decoded
        const CASES: &[(&str, &str, &str)] = &[
            ("no escape", "p { color: red }", "p { color: red }"),
            ("hex with its closing space", "u\\72 l(", "url("),
            ("hex without a space", "u\\72l(", "url("),
            ("hex of six digits", "\\000075rl(", "url("),
            ("hex in capitals", "\\55RL(", "URL("),
            ("two escapes in a row", "\\75\\72\\6c(", "url("),
            ("a non-hex letter escaped", "u\\rl(", "url("),
            ("a hex letter escaped as itself is hex", "\\a", "\\a"),
            ("at rule", "@\\69mport", "@import"),
            ("a hyphen", "\\2d webkit-image-set(", "-webkit-image-set("),
            ("a tab closes the escape", "\\75\trl(", "url("),
            ("crlf closes the escape", "\\75\r\nrl(", "url("),
            ("only one space is eaten", "\\75  rl(", "u rl("),
            ("a quote stays escaped", "'a\\'b'", "'a\\'b'"),
            ("a hex quote stays escaped", "'a\\27 b'", "'a\\27 b'"),
            ("a parenthesis stays escaped", "u\\29 ", "u\\29 "),
            ("a backslash stays escaped", "\\\\72", "\\\\72"),
            ("a newline stays escaped", "a\\\nb", "a\\\nb"),
            ("a lone trailing backslash", "a\\", "a\\"),
            ("a code point beyond unicode", "\\ffffff x", "\\ffffff x"),
            ("an escaped multibyte char stays", "\\\u{e9}x", "\\\u{e9}x"),
            ("nul stays", "\\0 x", "\\0 x"),
        ];
        for (name, css, want) in CASES {
            assert_eq!(unescaped(css), *want, "{name}");
        }
    }
}
