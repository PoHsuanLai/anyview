//! The pass over a Markdown document's events that makes it safe and complete for a sealed frame:
//! raw HTML becomes text, link targets are vetted, local images are inlined, code blocks are
//! highlighted and headings get their anchors.

use super::images::{LocalFiles, data_url};
use super::links::safe_link;
use super::outline::{Anchors, Heading, HeadingLevel};
use crate::code::{Highlighter, tokens_html};
use crate::escape::escape_into;
use anyview_core::FilePath;
use pulldown_cmark::{CodeBlockKind, CowStr, Event, HeadingLevel as Level, Tag, TagEnd};

/// What rendering may use besides the document: where its local files are, how to read them, and
/// how to highlight code.
#[derive(Clone, Copy)]
pub struct RenderEnv<'a> {
    /// The document's directory, which relative image references are taken from.
    pub base: Option<&'a FilePath>,
    /// Reads local image files.
    pub files: &'a dyn LocalFiles,
    /// Highlights fenced code; with `None` code blocks are plain.
    pub highlighter: Option<&'a Highlighter>,
}

impl std::fmt::Debug for RenderEnv<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderEnv")
            .field("base", &self.base)
            .field("highlighter", &self.highlighter.is_some())
            .finish_non_exhaustive()
    }
}

/// How an image's closing event is written.
#[derive(Clone, Copy)]
enum ImageEnd {
    /// The image has a source: close the image.
    Kept,
    /// The image has none: close the span that holds its alt text.
    Dropped,
}

fn level_of(level: Level) -> HeadingLevel {
    HeadingLevel::clamped(level as u8) // the levels are numbered 1 to 6
}

/// The plain text of the events after a heading's start, up to its end.
fn text_until_heading_end(events: &[Event<'_>]) -> String {
    let mut text = String::new();
    for event in events {
        match event {
            Event::End(TagEnd::Heading(_)) => break,
            Event::Text(t) | Event::Code(t) => text.push_str(t),
            Event::SoftBreak | Event::HardBreak => text.push(' '),
            Event::Start(_)
            | Event::End(_)
            | Event::InlineMath(_)
            | Event::DisplayMath(_)
            | Event::Html(_)
            | Event::InlineHtml(_)
            | Event::FootnoteReference(_)
            | Event::Rule
            | Event::TaskListMarker(_) => {}
        }
    }
    text
}

/// The text inside a code block whose start is at `events[0]`'s successor, and how many events it
/// spans including the end.
fn code_block(events: &[Event<'_>]) -> (String, usize) {
    let mut text = String::new();
    for (i, event) in events.iter().enumerate() {
        match event {
            Event::End(TagEnd::CodeBlock) => return (text, i + 1),
            Event::Text(t) => text.push_str(t),
            Event::Start(_)
            | Event::End(_)
            | Event::Code(_)
            | Event::InlineMath(_)
            | Event::DisplayMath(_)
            | Event::Html(_)
            | Event::InlineHtml(_)
            | Event::FootnoteReference(_)
            | Event::SoftBreak
            | Event::HardBreak
            | Event::Rule
            | Event::TaskListMarker(_) => {}
        }
    }
    (text, events.len())
}

/// The first word of a fenced block's info string: `rust` of `rust,no_run`.
fn language(info: &str) -> &str {
    info.split(|c: char| c.is_whitespace() || c == ',')
        .next()
        .unwrap_or("")
}

/// The HTML of a fenced code block highlighted as its language, or `None` when the block names no
/// language the highlighter has.
fn highlighted(info: &str, text: &str, highlighter: &Highlighter) -> Option<String> {
    let label = language(info);
    let syntax = highlighter.syntax_by_token(label)?;
    let lines = highlighter.snippet(Some(syntax), text);
    let mut html = String::from("<pre><code class=\"language-");
    escape_into(&mut html, label);
    html.push_str("\">");
    html.push_str(&tokens_html(&lines));
    html.push_str("\n</code></pre>\n");
    Some(html)
}

fn html<'a>(text: &str) -> Event<'a> {
    Event::Html(CowStr::from(text.to_owned()))
}

/// The events made safe and complete, and the outline of the headings met on the way.
pub(super) fn prepare<'a>(
    events: Vec<Event<'a>>,
    env: &RenderEnv<'_>,
) -> (Vec<Event<'a>>, Vec<Heading>) {
    let mut out = Vec::with_capacity(events.len());
    let mut outline = Vec::new();
    let mut anchors = Anchors::default();
    let mut images: Vec<ImageEnd> = Vec::new();
    let mut i = 0;
    while i < events.len() {
        i += 1;
        let event = &events[i - 1];
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                let text = text_until_heading_end(&events[i..]);
                let anchor = anchors.next(&text);
                out.push(Event::Start(Tag::Heading {
                    level: *level,
                    id: Some(CowStr::from(anchor.as_str().to_owned())),
                    classes: Vec::new(),
                    attrs: Vec::new(),
                }));
                outline.push(Heading {
                    level: level_of(*level),
                    text,
                    anchor,
                });
            }
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }) => {
                out.push(Event::Start(Tag::Link {
                    link_type: *link_type,
                    dest_url: CowStr::from(safe_link(dest_url)),
                    title: title.clone(),
                    id: id.clone(),
                }));
            }
            Event::Start(Tag::Image {
                link_type,
                dest_url,
                title,
                id,
            }) => match data_url(dest_url, env.base, env.files) {
                Some(url) => {
                    images.push(ImageEnd::Kept);
                    out.push(Event::Start(Tag::Image {
                        link_type: *link_type,
                        dest_url: CowStr::from(url),
                        title: title.clone(),
                        id: id.clone(),
                    }));
                }
                None => {
                    images.push(ImageEnd::Dropped);
                    out.push(html("<span class=\"missing-image\">"));
                }
            },
            Event::End(TagEnd::Image) => match images.pop() {
                Some(ImageEnd::Dropped) => out.push(html("</span>")),
                Some(ImageEnd::Kept) | None => out.push(Event::End(TagEnd::Image)),
            },
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
                let (text, spanned) = code_block(&events[i..]);
                match env.highlighter.and_then(|h| highlighted(info, &text, h)) {
                    Some(block) => {
                        out.push(html(&block));
                        i += spanned;
                    }
                    None => out.push(event.clone()),
                }
            }
            Event::Start(Tag::HtmlBlock) => out.push(html("<pre class=\"raw-html\"><code>")),
            Event::End(TagEnd::HtmlBlock) => out.push(html("</code></pre>\n")),
            Event::Html(raw) | Event::InlineHtml(raw) => out.push(Event::Text(raw.clone())),
            Event::Start(_)
            | Event::End(_)
            | Event::Text(_)
            | Event::Code(_)
            | Event::InlineMath(_)
            | Event::DisplayMath(_)
            | Event::FootnoteReference(_)
            | Event::SoftBreak
            | Event::HardBreak
            | Event::Rule
            | Event::TaskListMarker(_) => out.push(event.clone()),
        }
    }
    (out, outline)
}
