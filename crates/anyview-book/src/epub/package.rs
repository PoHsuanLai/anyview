//! The package document: metadata, manifest and reading order.

use super::EpubMeta;
use crate::error::BookError;
use crate::zip_path::{directory_of, resolve};
use roxmltree::{Document, Node, ParsingOptions};

/// One chapter of the reading order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SpineItem {
    /// The entry that holds it.
    pub entry: String,
}

/// What the package document says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Package {
    pub meta: EpubMeta,
    pub spine: Vec<SpineItem>,
    pub cover: Option<String>,
    /// The entry of the EPUB 3 navigation document, when there is one.
    pub nav: Option<String>,
    /// The entry of the EPUB 2 contents document, when there is one.
    pub ncx: Option<String>,
}

/// Parses `bytes` as XML; a byte order mark and a DOCTYPE are tolerated.
pub(super) fn xml<'a>(part: &'static str, text: &'a str) -> Result<Document<'a>, BookError> {
    let options = ParsingOptions {
        allow_dtd: true,
        ..ParsingOptions::default()
    };
    Document::parse_with_options(text.trim_start_matches('\u{feff}'), options).map_err(|error| {
        BookError::Xml {
            part,
            reason: error.to_string(),
        }
    })
}

/// `bytes` as text.
pub(super) fn text_of(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// The entry of the package document the container names.
pub(super) fn rootfile(container: &[u8]) -> Result<String, BookError> {
    let text = text_of(container);
    let document = xml("the container", &text)?;
    document
        .descendants()
        .find(|node| node.has_tag_name("rootfile"))
        .and_then(|node| node.attribute("full-path"))
        .map(|path| path.trim_start_matches('/').to_owned())
        .ok_or(BookError::Missing("package document"))
}

fn children<'a, 'b>(node: Node<'a, 'b>, name: &'static str) -> impl Iterator<Item = Node<'a, 'b>> {
    node.children()
        .filter(move |child| child.has_tag_name(name))
}

fn words(node: Node<'_, '_>) -> String {
    node.descendants()
        .filter(Node::is_text)
        .filter_map(|text| text.text())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn metadata(root: Node<'_, '_>) -> EpubMeta {
    let scope = root
        .children()
        .find(|node| node.has_tag_name("metadata"))
        .unwrap_or(root);
    let all = |name: &'static str| -> Vec<String> {
        scope
            .descendants()
            .filter(|node| node.has_tag_name(name))
            .map(words)
            .filter(|text| !text.is_empty())
            .collect()
    };
    let first = |name: &'static str| all(name).into_iter().next();
    let authors = all("creator");
    EpubMeta {
        title: first("title"),
        author: (!authors.is_empty()).then(|| authors.join(", ")),
        publisher: first("publisher"),
        language: first("language"),
    }
}

/// A manifest item.
struct Item<'a> {
    id: &'a str,
    entry: Option<String>,
    media: &'a str,
    properties: &'a str,
}

fn manifest<'a>(root: Node<'a, 'a>, directory: &str) -> Vec<Item<'a>> {
    root.children()
        .filter(|node| node.has_tag_name("manifest"))
        .flat_map(|manifest| children(manifest, "item"))
        .map(|node| Item {
            id: node.attribute("id").unwrap_or(""),
            entry: node
                .attribute("href")
                .and_then(|href| resolve(directory, href)),
            media: node.attribute("media-type").unwrap_or(""),
            properties: node.attribute("properties").unwrap_or(""),
        })
        .collect()
}

/// The cover: the item the metadata names, the item flagged as the cover image, or the first
/// image in the manifest.
fn cover(root: Node<'_, '_>, items: &[Item<'_>]) -> Option<String> {
    let named = root
        .descendants()
        .filter(|node| node.has_tag_name("meta") && node.attribute("name") == Some("cover"))
        .find_map(|node| node.attribute("content"))
        .and_then(|id| items.iter().find(|item| item.id == id));
    let flagged = items.iter().find(|item| {
        item.properties
            .split_whitespace()
            .any(|p| p == "cover-image")
    });
    let image = items.iter().find(|item| item.media.starts_with("image/"));
    named
        .or(flagged)
        .or(image)
        .and_then(|item| item.entry.clone())
}

pub(super) fn parse(package_path: &str, bytes: &[u8]) -> Result<Package, BookError> {
    let text = text_of(bytes);
    let document = xml("the package document", &text)?;
    let root = document.root_element();
    let directory = directory_of(package_path);
    let items = manifest(root, directory);
    let spine_node = root.children().find(|node| node.has_tag_name("spine"));
    let spine = spine_node
        .into_iter()
        .flat_map(|spine| children(spine, "itemref"))
        .filter_map(|itemref| itemref.attribute("idref"))
        .filter_map(|id| items.iter().find(|item| item.id == id))
        .filter(|item| item.media.contains("html"))
        .filter_map(|item| item.entry.clone())
        .map(|entry| SpineItem { entry })
        .collect();
    let by_property = |property: &str| {
        items
            .iter()
            .find(|item| item.properties.split_whitespace().any(|p| p == property))
            .and_then(|item| item.entry.clone())
    };
    let ncx = spine_node
        .and_then(|spine| spine.attribute("toc"))
        .and_then(|id| items.iter().find(|item| item.id == id))
        .and_then(|item| item.entry.clone())
        .or_else(|| {
            items
                .iter()
                .find(|item| item.media == "application/x-dtbncx+xml")
                .and_then(|item| item.entry.clone())
        });
    Ok(Package {
        meta: metadata(root),
        spine,
        cover: cover(root, &items),
        nav: by_property("nav"),
        ncx,
    })
}
