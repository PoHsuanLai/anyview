//! The file card: one design for what a file is, drawn in the Info panel and as the stage of a file
//! the viewer has no full tier for. An icon or a small picture, the name, one summary line
//! (`JPEG image · 4032 × 3024 · 3.2 MB`), chips for the notable traits, and a "Details"
//! disclosure that opens the rest as stacked captions: a header for each group, and each fact as a
//! quiet caption over its value, left-aligned with no label column.

use anyview_core::{FactLabel, Facts, FormatKind};
use dioxus::prelude::*;
use ds::components::content::image_source::ImageSource;
use ds::components::controls::chip::{Chip, ChipVariant};
use ds::components::controls::disclosure::Disclosure;
use ds::prelude::{Icon, IconSource, IconView, SectionHeader, Shown};
use ds::style::icon::render::{IconPx, IconSize};
use ds_core::word::Word;

/// The side of the card's icon, in logical pixels.
const GLYPH_SIDE: u8 = 64;

/// The icon of a file of `kind`.
fn icon_of(kind: Option<FormatKind>) -> Icon {
    match kind {
        Some(FormatKind::Raster | FormatKind::Vector) => Icon::Image,
        Some(FormatKind::Audio) => Icon::Music,
        Some(FormatKind::Folder) => Icon::Folder,
        Some(
            FormatKind::Pdf
            | FormatKind::PlainText
            | FormatKind::Markdown
            | FormatKind::Code
            | FormatKind::Table
            | FormatKind::Tree
            | FormatKind::Video
            | FormatKind::Book
            | FormatKind::Font
            | FormatKind::Archive
            | FormatKind::Office
            | FormatKind::Other,
        )
        | None => Icon::File,
    }
}

/// The notable traits of a file, as chips: what its facts say about it beyond the summary.
pub(crate) fn chips_of(facts: &Facts) -> Vec<&'static str> {
    let mut chips = Vec::new();
    if facts.value(FactLabel::Frames).is_some() {
        chips.push("Animated");
    }
    chips
}

/// The card of the file called `name`, of `kind`, whose rows are `facts`. `thumbnail` is a small
/// picture of the file when it carries one, in place of the icon. `actions` are the buttons under
/// the card (Show in Folder). `note` is a line under the summary.
#[component]
pub(crate) fn InfoCard(
    name: String,
    kind: Option<FormatKind>,
    facts: Facts,
    #[props(default)] thumbnail: Option<ImageSource>,
    #[props(default)] note: Option<String>,
    #[props(default)] actions: Option<Element>,
) -> Element {
    let mut details = use_signal(|| Shown::Visible);
    let summary = facts.summary();
    let chips = chips_of(&facts);
    let sections = facts.sections();
    let titled = sections.len() > 1;
    rsx! {
        div { class: "viewer-card",
            if let Some(picture) = thumbnail {
                img { class: "viewer-card-picture", alt: "Preview", src: picture.0 }
            } else {
                div { class: "viewer-card-glyph",
                    IconView {
                        source: IconSource::from(icon_of(kind)),
                        size: IconSize::Px(IconPx(GLYPH_SIDE)),
                    }
                }
            }
            h2 { class: "viewer-card-name", "{name}" }
            if let Some(line) = summary {
                p { class: "viewer-card-summary", "{line}" }
            }
            if !chips.is_empty() {
                div { class: "viewer-card-chips",
                    for chip in chips {
                        Chip { variant: ChipVariant::Neutral, text: chip.to_owned() }
                    }
                }
            }
            if let Some(note) = note {
                p { class: "viewer-card-note", "{note}" }
            }
            if let Some(actions) = actions {
                {actions}
            }
            if !sections.is_empty() {
                div { class: "viewer-card-details",
                    Disclosure {
                        shown: details(),
                        label: Some("Details".to_owned()),
                        on_toggle: move |next: Shown| details.set(next),
                        for (group , rows) in sections {
                            Fragment { key: "{group.slug()}",
                                if titled {
                                    SectionHeader { title: group.label().to_string() }
                                }
                                dl { class: "viewer-facts",
                                    for row in rows {
                                        div { class: "viewer-fact",
                                            dt { class: "viewer-fact-caption", "{row.label.label()}" }
                                            dd { class: "viewer-fact-value", "{row.value.as_str()}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::FactValue;

    #[test]
    fn only_an_animation_is_marked_as_one() {
        let still = Facts::empty().with(FactLabel::Dimensions, FactValue::text("4 × 4"));
        let moving = still
            .clone()
            .with(FactLabel::Frames, FactValue::text("12 frames"));
        assert!(chips_of(&still).is_empty());
        assert_eq!(chips_of(&moving), ["Animated"]);
    }
}
