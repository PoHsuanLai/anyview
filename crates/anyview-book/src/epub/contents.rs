//! The table of contents: the EPUB 3 navigation document, else the EPUB 2 contents file, else the
//! reading order itself.

use super::package::{Package, text_of, xml};
use crate::zip_path::{directory_of, resolve};
use anyview_core::{ByteLen, Input, SectionIndex};
use roxmltree::Node;
use std::collections::HashMap;

const PART_LIMIT: ByteLen = ByteLen(8 * 1024 * 1024);

/// One line of the contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TocEntry {
    /// What the line says.
    pub title: String,
    /// How deep it is nested, from zero.
    pub depth: u8,
    /// The chapter it leads to, when the reading order has it.
    pub section: Option<SectionIndex>,
}

/// The contents of the book at `input`. Never empty: a book whose contents cannot be read lists
/// its chapters by number.
pub(super) fn read(input: &Input, package: &Package) -> Vec<TocEntry> {
    let from_nav = package
        .nav
        .as_deref()
        .and_then(|entry| nav(input, package, entry));
    let found = from_nav.or_else(|| {
        package
            .ncx
            .as_deref()
            .and_then(|entry| ncx(input, package, entry))
    });
    match found {
        Some(entries) if !entries.is_empty() => entries,
        Some(_) | None => (0..package.spine.len())
            .map(|at| TocEntry {
                title: format!("Chapter {}", at + 1),
                depth: 0,
                section: u32::try_from(at).ok().map(SectionIndex),
            })
            .collect(),
    }
}

/// The chapter of each spine entry; the first of two with the same entry wins.
type Spine<'a> = HashMap<&'a str, usize>;

fn spine_index(package: &Package) -> Spine<'_> {
    let mut map = HashMap::with_capacity(package.spine.len());
    for (at, item) in package.spine.iter().enumerate() {
        map.entry(item.entry.as_str()).or_insert(at);
    }
    map
}

fn section_of(spine: &Spine<'_>, base: &str, href: &str) -> Option<SectionIndex> {
    let entry = resolve(base, href)?;
    let at = *spine.get(entry.as_str())?;
    u32::try_from(at).ok().map(SectionIndex)
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

fn nav(input: &Input, package: &Package, entry: &str) -> Option<Vec<TocEntry>> {
    let text = text_of(&read_part(input, entry)?);
    let document = xml("the navigation document", &text).ok()?;
    let navs: Vec<Node<'_, '_>> = document
        .descendants()
        .filter(|node| node.has_tag_name("nav"))
        .collect();
    let toc = navs
        .iter()
        .find(|node| {
            node.attributes()
                .any(|a| a.name() == "type" && a.value().split_whitespace().any(|v| v == "toc"))
        })
        .or_else(|| navs.first())?;
    let list = toc.children().find(|node| node.has_tag_name("ol"))?;
    let mut entries = Vec::new();
    nav_list(
        list,
        0,
        &spine_index(package),
        directory_of(entry),
        &mut entries,
    );
    Some(entries)
}

fn nav_list(list: Node<'_, '_>, depth: u8, spine: &Spine<'_>, base: &str, out: &mut Vec<TocEntry>) {
    for item in list.children().filter(|node| node.has_tag_name("li")) {
        let label = item
            .children()
            .find(|node| node.has_tag_name("a") || node.has_tag_name("span"));
        if let Some(label) = label {
            let title = words(label);
            let section = label
                .attribute("href")
                .and_then(|href| section_of(spine, base, href));
            if !title.is_empty() {
                out.push(TocEntry {
                    title,
                    depth,
                    section,
                });
            }
        }
        if let Some(nested) = item.children().find(|node| node.has_tag_name("ol")) {
            nav_list(nested, depth.saturating_add(1), spine, base, out);
        }
    }
}

fn ncx(input: &Input, package: &Package, entry: &str) -> Option<Vec<TocEntry>> {
    let text = text_of(&read_part(input, entry)?);
    let document = xml("the contents file", &text).ok()?;
    let map = document
        .descendants()
        .find(|node| node.has_tag_name("navMap"))?;
    let mut entries = Vec::new();
    ncx_points(
        map,
        0,
        &spine_index(package),
        directory_of(entry),
        &mut entries,
    );
    Some(entries)
}

fn ncx_points(
    parent: Node<'_, '_>,
    depth: u8,
    spine: &Spine<'_>,
    base: &str,
    out: &mut Vec<TocEntry>,
) {
    for point in parent
        .children()
        .filter(|node| node.has_tag_name("navPoint"))
    {
        let title = point
            .children()
            .find(|node| node.has_tag_name("navLabel"))
            .map(words)
            .unwrap_or_default();
        let section = point
            .children()
            .find(|node| node.has_tag_name("content"))
            .and_then(|content| content.attribute("src"))
            .and_then(|src| section_of(spine, base, src));
        if !title.is_empty() {
            out.push(TocEntry {
                title,
                depth,
                section,
            });
        }
        ncx_points(point, depth.saturating_add(1), spine, base, out);
    }
}

fn read_part(input: &Input, entry: &str) -> Option<Vec<u8>> {
    crate::zip_read::read(input, entry, PART_LIMIT).ok()
}
