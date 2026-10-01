//! Escaping text for HTML, the one place it is written.

/// `text` with `&`, `<`, `>`, `"` and `'` replaced by character references, appended to `out`.
pub(crate) fn escape_into(out: &mut String, text: &str) {
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markup_characters_are_escaped_and_the_rest_is_kept() {
        const CASES: &[(&str, &str)] = &[
            ("plain", "plain"),
            ("<b>&</b>", "&lt;b&gt;&amp;&lt;/b&gt;"),
            ("say \"hi\"", "say &quot;hi&quot;"),
            ("it's", "it&#39;s"),
            ("café €", "café €"),
            ("", ""),
        ];
        for (text, want) in CASES {
            let mut out = String::new();
            escape_into(&mut out, text);
            assert_eq!(out, *want, "{text}");
        }
    }
}
