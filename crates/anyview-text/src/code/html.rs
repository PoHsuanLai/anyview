//! Highlighted lines as HTML: each styled span wrapped in an element whose class names the token
//! class. The page's stylesheet maps `tok-keyword` and the rest to colour tokens; no colour is
//! written here.

use super::{TokenClass, TokenLine};
use crate::escape::escape_into;
use ds_core::word::Word;

/// The class prefix of a token span: a `Keyword` is `class="tok-keyword"`.
pub const TOKEN_CLASS_PREFIX: &str = "tok-";

/// The lines as HTML text, joined by line feeds. Plain spans are bare text; every other span is a
/// `<span class="tok-…">`.
pub fn tokens_html(lines: &[TokenLine]) -> String {
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        for span in &line.spans {
            if span.class == TokenClass::Plain {
                escape_into(&mut out, &span.text);
            } else {
                out.push_str("<span class=\"");
                out.push_str(TOKEN_CLASS_PREFIX);
                out.push_str(span.class.slug());
                out.push_str("\">");
                escape_into(&mut out, &span.text);
                out.push_str("</span>");
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code::TokenSpan;
    use anyview_core::LineIndex;

    fn line(number: u32, spans: &[(TokenClass, &str)]) -> TokenLine {
        TokenLine {
            number: LineIndex(number),
            spans: spans
                .iter()
                .map(|(class, text)| TokenSpan {
                    class: *class,
                    text: (*text).to_owned(),
                })
                .collect(),
        }
    }

    #[test]
    fn spans_are_wrapped_by_class_escaped_and_lines_joined() {
        let lines = [
            line(
                0,
                &[(TokenClass::Keyword, "if"), (TokenClass::Plain, " a < b {")],
            ),
            line(1, &[(TokenClass::String, "\"x\""), (TokenClass::Plain, "")]),
        ];
        assert_eq!(
            tokens_html(&lines),
            "<span class=\"tok-keyword\">if</span> a &lt; b {\n<span class=\"tok-string\">&quot;x&quot;</span>"
        );
        assert_eq!(tokens_html(&[]), "");
    }
}
