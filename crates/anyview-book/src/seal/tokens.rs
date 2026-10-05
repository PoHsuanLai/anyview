//! A forgiving HTML tokenizer: tags, text and the raw text of `script`, `style` and their kin.
//! It decides nothing about what is allowed; the sealer does.

/// One piece of a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Token<'a> {
    /// Text between tags, as written (entities undecoded).
    Text(&'a str),
    /// An opening tag: the lower-cased name and the attributes, names lower-cased and values as
    /// written between their quotes.
    Open {
        name: String,
        attributes: Vec<(String, String)>,
    },
    /// A closing tag.
    Close { name: String },
    /// The contents of a `script`, `style`, `title` or `textarea`, which are not markup.
    Raw { name: String, text: &'a str },
}

/// The elements whose contents are text up to their own closing tag.
const RAW_TEXT: &[&str] = &["script", "style", "title", "textarea"];

/// The position of `needle` (lower-case ASCII) in `hay`, ignoring the case of `hay`.
fn find_ci(hay: &str, needle: &str) -> Option<usize> {
    let (hay, needle) = (hay.as_bytes(), needle.as_bytes());
    hay.windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle))
}

/// Every token of `html`, in order. Comments, doctypes, processing instructions and CDATA are
/// dropped.
pub(super) fn tokens(html: &str) -> Vec<Token<'_>> {
    let mut out = Vec::new();
    let mut rest = html;
    while !rest.is_empty() {
        let Some(lt) = rest.find('<') else {
            out.push(Token::Text(rest));
            break;
        };
        if lt > 0 {
            out.push(Token::Text(&rest[..lt]));
            rest = &rest[lt..];
        }
        rest = markup(rest, &mut out);
    }
    out
}

/// Reads the markup at the start of `rest` (which begins with `<`) into `out`, and returns what
/// follows it.
fn markup<'a>(rest: &'a str, out: &mut Vec<Token<'a>>) -> &'a str {
    let after = &rest[1..];
    if let Some(body) = after.strip_prefix("!--") {
        return body.find("-->").map_or("", |end| &body[end + 3..]);
    }
    if after.starts_with("![CDATA[") {
        return after.find("]]>").map_or("", |end| &after[end + 3..]);
    }
    if after.starts_with('!') || after.starts_with('?') {
        return after.find('>').map_or("", |end| &after[end + 1..]);
    }
    if let Some(closing) = after.strip_prefix('/') {
        return close_tag(closing, out).unwrap_or_else(|| stray(rest, out));
    }
    if after.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return open_tag(after, out);
    }
    stray(rest, out)
}

/// A `<` that starts no markup is text.
fn stray<'a>(rest: &'a str, out: &mut Vec<Token<'a>>) -> &'a str {
    out.push(Token::Text("&lt;"));
    &rest[1..]
}

fn close_tag<'a>(after: &'a str, out: &mut Vec<Token<'a>>) -> Option<&'a str> {
    if !after.starts_with(|c: char| c.is_ascii_alphabetic()) {
        return None;
    }
    let end = after.find('>')?;
    let name = after[..end]
        .split(|c: char| c.is_ascii_whitespace())
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    out.push(Token::Close { name });
    Some(&after[end + 1..])
}

fn open_tag<'a>(after: &'a str, out: &mut Vec<Token<'a>>) -> &'a str {
    let name_end = after
        .find(|c: char| c.is_ascii_whitespace() || c == '/' || c == '>')
        .unwrap_or(after.len());
    let name = after[..name_end].to_ascii_lowercase();
    let (attributes, rest) = attributes(&after[name_end..]);
    if RAW_TEXT.contains(&name.as_str()) {
        let closing = format!("</{name}");
        let (text, tail) = match find_ci(rest, &closing) {
            Some(at) => {
                let tail = &rest[at..];
                (
                    &rest[..at],
                    tail.find('>').map_or("", |end| &tail[end + 1..]),
                )
            }
            None => (rest, ""),
        };
        out.push(Token::Open {
            name: name.clone(),
            attributes,
        });
        out.push(Token::Raw {
            name: name.clone(),
            text,
        });
        out.push(Token::Close { name });
        return tail;
    }
    out.push(Token::Open { name, attributes });
    rest
}

/// The attributes at the start of `text` up to the tag's `>`, and what follows the tag.
fn attributes(text: &str) -> (Vec<(String, String)>, &str) {
    let mut found = Vec::new();
    let mut rest = text;
    loop {
        rest = rest.trim_start_matches(|c: char| c.is_ascii_whitespace() || c == '/');
        let Some(first) = rest.chars().next() else {
            return (found, "");
        };
        if first == '>' {
            return (found, &rest[1..]);
        }
        let name_end = rest
            .find(|c: char| c.is_ascii_whitespace() || matches!(c, '=' | '>' | '/'))
            .unwrap_or(rest.len())
            .max(first.len_utf8());
        let name = rest[..name_end].to_ascii_lowercase();
        rest = rest[name_end..].trim_start();
        let Some(valued) = rest.strip_prefix('=') else {
            found.push((name, String::new()));
            continue;
        };
        let (value, tail) = value(valued.trim_start());
        found.push((name, value.to_owned()));
        rest = tail;
    }
}

/// An attribute value, quoted or bare, and what follows it.
fn value(text: &str) -> (&str, &str) {
    match text.chars().next() {
        Some(quote @ ('"' | '\'')) => {
            let inner = &text[1..];
            match inner.find(quote) {
                Some(end) => (&inner[..end], &inner[end + 1..]),
                None => (inner, ""),
            }
        }
        _ => {
            let end = text
                .find(|c: char| c.is_ascii_whitespace() || c == '>')
                .unwrap_or(text.len());
            (&text[..end], &text[end..])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(name: &str, attributes: &[(&str, &str)]) -> Token<'static> {
        Token::Open {
            name: name.to_owned(),
            attributes: attributes
                .iter()
                .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
                .collect(),
        }
    }

    #[test]
    fn markup_is_cut_into_tokens() {
        // name, html, tokens
        let cases: Vec<(&str, &str, Vec<Token<'static>>)> = vec![
            (
                "quoted, bare and valueless attributes",
                "<a href=\"x y\" id='b' class=c hidden>",
                vec![open(
                    "a",
                    &[("href", "x y"), ("id", "b"), ("class", "c"), ("hidden", "")],
                )],
            ),
            (
                "case is folded in names",
                "<IMG SRC=a.png /></P>",
                vec![
                    open("img", &[("src", "a.png")]),
                    Token::Close { name: "p".into() },
                ],
            ),
            (
                "a comment and a doctype vanish",
                "a<!-- <b> -->b<!DOCTYPE html>c",
                vec![Token::Text("a"), Token::Text("b"), Token::Text("c")],
            ),
            (
                "script contents are raw",
                "<script>if (a<b) x()</script>t",
                vec![
                    open("script", &[]),
                    Token::Raw {
                        name: "script".into(),
                        text: "if (a<b) x()",
                    },
                    Token::Close {
                        name: "script".into(),
                    },
                    Token::Text("t"),
                ],
            ),
            (
                "a lone less-than is text",
                "1 < 2",
                vec![Token::Text("1 "), Token::Text("&lt;"), Token::Text(" 2")],
            ),
            (
                "an unclosed tag at the end ends the document",
                "<p>x<b class=\"y",
                vec![
                    open("p", &[]),
                    Token::Text("x"),
                    open("b", &[("class", "y")]),
                ],
            ),
        ];
        for (name, html, want) in cases {
            assert_eq!(tokens(html), want, "{name}");
        }
    }
}
