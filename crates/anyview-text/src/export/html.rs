//! The HTML page a text document is printed from.

use crate::code::{Highlighter, tokens_html};
use crate::encoding::{Coverage, detect};
use crate::error::TextError;
use crate::escape::escape_into;
use crate::markdown::{DiskFiles, RenderEnv, render};
use anyview_core::{FilePath, HtmlDoc};

/// The largest file that is printed: the page is built in memory and laid out whole.
const MAX_PRINTED_BYTES: u64 = 16 * 1024 * 1024;

/// The page's stylesheet. Paper is white, so the colours are fixed rather than the window's
/// tokens; the token classes are the ones `tokens_html` and the Markdown renderer write.
const PAGE_STYLE: &str = include_str!("print.css");

/// `doc` as a whole HTML page, ready to print: Markdown is rendered (a local image becomes a
/// `data:` URL, since the printer loads nothing else), source code is highlighted, plain text is
/// kept as it is. Blocking: reads the file.
pub fn printable_html(doc: &HtmlDoc, highlighter: &Highlighter) -> Result<String, TextError> {
    let (file, body) = match doc {
        HtmlDoc::Markdown(file) => {
            let text = read_text(file)?;
            let base = file.parent();
            let env = RenderEnv {
                base: base.as_ref(),
                files: &DiskFiles,
                highlighter: Some(highlighter),
            };
            (file, render(&text, &env).html)
        }
        HtmlDoc::Code { file, syntax } => {
            let lines = highlighter.snippet(highlighter.syntax(syntax), &read_text(file)?);
            (file, pre(&tokens_html(&lines)))
        }
        HtmlDoc::PlainText(file) => {
            let mut escaped = String::new();
            escape_into(&mut escaped, &read_text(file)?);
            (file, pre(&escaped))
        }
    };
    let mut title = String::new();
    escape_into(
        &mut title,
        &file
            .file_name()
            .map_or_else(String::new, |name| name.as_str().to_owned()),
    );
    Ok(format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>{title}</title>\
         <style>{PAGE_STYLE}</style></head><body>{body}</body></html>"
    ))
}

fn pre(escaped: &str) -> String {
    format!("<pre class=\"source\">{escaped}</pre>")
}

/// The text of `file`, decoded as its byte-order mark or content says.
fn read_text(file: &FilePath) -> Result<String, TextError> {
    let read = |kind: std::io::ErrorKind| TextError::Read {
        path: file.as_path().to_path_buf(),
        kind,
    };
    let len = std::fs::metadata(file.as_path())
        .map_err(|error| read(error.kind()))?
        .len();
    if len > MAX_PRINTED_BYTES {
        return Err(TextError::TooLargeToPrint { len });
    }
    let bytes = std::fs::read(file.as_path()).map_err(|error| read(error.kind()))?;
    let found = detect(&bytes, Coverage::Whole);
    Ok(found
        .codec
        .decode(&bytes[usize::from(found.mark)..])
        .into_owned())
}
