//! The walk over a chapter's tokens that writes the sealed markup.

use super::Chapter;
use super::assets::Assets;
use super::css::seal_css;
use super::element::{
    ATTRIBUTES, DROPPED_WITH_CONTENT, ELEMENTS, FOREIGN, VOID, is_length, is_safe_link,
};
use super::entities::{decode, escape};
use super::tokens::{Token, tokens};
use crate::zip_path::{directory_of, resolve};

/// What a walk keeps while it writes.
struct Sealer<'a> {
    directory: &'a str,
    assets: &'a dyn Assets,
    styles: String,
    body: String,
    /// The elements written and not yet closed, innermost last.
    open: Vec<&'static str>,
    /// How deep inside an element that is left out with its contents.
    dropped: u32,
    /// How deep inside SVG or MathML.
    foreign: u32,
}

pub(super) fn seal(html: &str, directory: &str, assets: &dyn Assets) -> Chapter {
    let mut sealer = Sealer {
        directory,
        assets,
        styles: String::new(),
        body: String::new(),
        open: Vec::new(),
        dropped: 0,
        foreign: 0,
    };
    for token in tokens(html.trim_start_matches('\u{feff}')) {
        sealer.token(token);
    }
    while let Some(name) = sealer.open.pop() {
        sealer.body.push_str(&format!("</{name}>"));
    }
    Chapter {
        styles: sealer.styles,
        body: sealer.body,
    }
}

fn known(name: &str) -> Option<&'static str> {
    ELEMENTS.iter().copied().find(|element| *element == name)
}

fn attribute<'v>(attributes: &'v [(String, String)], name: &str) -> Option<&'v str> {
    attributes
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

impl Sealer<'_> {
    fn token(&mut self, token: Token<'_>) {
        match token {
            Token::Text(text) if self.dropped == 0 && self.foreign == 0 => {
                self.body.push_str(text);
            }
            Token::Text(_) => {}
            Token::Raw { name, text } => self.raw(&name, text),
            Token::Open { name, attributes } => self.opened(&name, &attributes),
            Token::Close { name } => self.closed(&name),
        }
    }

    fn raw(&mut self, name: &str, text: &str) {
        if name == "style" && self.dropped == 0 {
            self.styles
                .push_str(&seal_css(text, self.directory, self.assets));
            self.styles.push('\n');
        }
    }

    fn opened(&mut self, name: &str, attributes: &[(String, String)]) {
        if FOREIGN.contains(&name) {
            self.foreign += 1;
            return;
        }
        if self.foreign > 0 {
            if name == "image" {
                self.svg_image(attributes);
            }
            return;
        }
        if DROPPED_WITH_CONTENT.contains(&name) {
            self.dropped += 1;
            return;
        }
        if self.dropped > 0 {
            return;
        }
        if name == "link" {
            self.stylesheet(attributes);
        } else if let Some(element) = known(name) {
            self.element(element, attributes);
        }
    }

    fn closed(&mut self, name: &str) {
        if FOREIGN.contains(&name) {
            self.foreign = self.foreign.saturating_sub(1);
        } else if DROPPED_WITH_CONTENT.contains(&name) {
            self.dropped = self.dropped.saturating_sub(1);
        } else if self.dropped == 0
            && self.foreign == 0
            && !VOID.contains(&name)
            && self.open.contains(&name)
        {
            while let Some(open) = self.open.pop() {
                self.body.push_str(&format!("</{open}>"));
                if open == name {
                    break;
                }
            }
        }
    }

    fn element(&mut self, name: &'static str, attributes: &[(String, String)]) {
        if name == "img" {
            self.image(attributes);
            return;
        }
        self.body.push('<');
        self.body.push_str(name);
        self.attributes(name, attributes);
        self.body.push('>');
        if !VOID.contains(&name) {
            self.open.push(name);
        }
    }

    fn attributes(&mut self, element: &str, attributes: &[(String, String)]) {
        for (key, value) in attributes {
            let value = decode(value);
            let written = match key.as_str() {
                "style" => Some(seal_css(&value, self.directory, self.assets)),
                "href" if element == "a" && is_safe_link(&value) => Some(value),
                "width" | "height" if is_length(&value) => Some(value),
                key if ATTRIBUTES.contains(&key) => Some(value),
                _ => None,
            };
            if let Some(value) = written {
                self.body
                    .push_str(&format!(" {key}=\"{}\"", escape(&value)));
            }
        }
    }

    /// An `<img>`: its picture inlined, or its alt text when the package does not hold it.
    fn image(&mut self, attributes: &[(String, String)]) {
        let source = attribute(attributes, "src").map(decode);
        let data = source
            .and_then(|source| resolve(self.directory, &source))
            .and_then(|entry| self.assets.data_url(&entry));
        match data {
            Some(data) => {
                self.body
                    .push_str(&format!("<img src=\"{}\"", escape(&data)));
                self.attributes("img", attributes);
                self.body.push('>');
            }
            None => {
                if let Some(alt) = attribute(attributes, "alt") {
                    self.body.push_str(&escape(&decode(alt)));
                }
            }
        }
    }

    /// An SVG `<image>` (a cover page is often only that) as a picture.
    fn svg_image(&mut self, attributes: &[(String, String)]) {
        let href = attribute(attributes, "href").or_else(|| attribute(attributes, "xlink:href"));
        let data = href
            .map(decode)
            .and_then(|href| resolve(self.directory, &href))
            .and_then(|entry| self.assets.data_url(&entry));
        if let Some(data) = data {
            self.body.push_str(&format!(
                "<img src=\"{}\" style=\"max-width:100%\">",
                escape(&data)
            ));
        }
    }

    /// A `<link rel="stylesheet">`: the sheet sealed and added to the styles.
    fn stylesheet(&mut self, attributes: &[(String, String)]) {
        let is_sheet = attribute(attributes, "rel").is_some_and(|rel| {
            rel.split_ascii_whitespace()
                .any(|word| word.eq_ignore_ascii_case("stylesheet"))
        });
        let entry = attribute(attributes, "href")
            .map(decode)
            .and_then(|href| resolve(self.directory, &href));
        let (true, Some(entry)) = (is_sheet, entry) else {
            return;
        };
        if let Some(text) = self.assets.text(&entry) {
            self.styles
                .push_str(&seal_css(&text, directory_of(&entry), self.assets));
            self.styles.push('\n');
        }
    }
}
