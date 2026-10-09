use super::*;
use crate::source::ByteLen;

#[test]
fn rows_keep_their_order_and_are_found_by_label() {
    let facts = Facts::empty()
        .with(FactLabel::Kind, FactValue::text("PNG image"))
        .with(FactLabel::Size, FactValue::size(ByteLen(2_000)))
        .with(FactLabel::Kind, FactValue::text("shadowed"));
    let labels: Vec<_> = facts.rows().iter().map(|f| f.label).collect();
    assert_eq!(labels, [FactLabel::Kind, FactLabel::Size, FactLabel::Kind]);
    assert_eq!(
        facts.value(FactLabel::Kind),
        Some(&FactValue::text("PNG image"))
    );
    assert_eq!(
        facts.value(FactLabel::Size).map(FactValue::as_str),
        Some("2.0 KB")
    );
    assert_eq!(facts.value(FactLabel::Pages), None);
    assert!(Facts::empty().rows().is_empty());
    assert_eq!(Facts::default(), Facts::empty());
}

#[test]
fn a_row_that_names_no_section_goes_where_its_label_does() {
    let row = Fact::new(FactLabel::Pages, FactValue::text("3 pages"));
    assert_eq!((row.group, row.tier), (FactGroup::Document, Tier::Headline));
    let row = Fact::new(FactLabel::Camera, FactValue::text("Canon"));
    assert_eq!((row.group, row.tier), (FactGroup::Camera, Tier::Detail));
    let moved = row.in_group(FactGroup::Video).at_tier(Tier::Headline);
    assert_eq!(
        (moved.group, moved.tier),
        (FactGroup::Video, Tier::Headline)
    );
}

#[test]
fn sections_come_in_display_order_with_general_last() {
    let facts = Facts::empty()
        .with(FactLabel::Kind, FactValue::text("JPEG image"))
        .with(FactLabel::Camera, FactValue::text("Canon"))
        .with(FactLabel::Colour, FactValue::text("RGB 8-bit"))
        .with(FactLabel::Lens, FactValue::text("50 mm"))
        .with(FactLabel::Modified, FactValue::text("today"));
    let shape: Vec<(FactGroup, Vec<FactLabel>)> = facts
        .sections()
        .into_iter()
        .map(|(group, rows)| (group, rows.iter().map(|r| r.label).collect()))
        .collect();
    assert_eq!(
        shape,
        [
            (FactGroup::Picture, vec![FactLabel::Colour]),
            (FactGroup::Camera, vec![FactLabel::Camera, FactLabel::Lens]),
            (
                FactGroup::General,
                vec![FactLabel::Kind, FactLabel::Modified]
            ),
        ]
    );
    assert!(Facts::empty().sections().is_empty());
}

#[test]
fn a_later_producer_replaces_the_rows_it_also_has() {
    let family = Facts::empty()
        .with(FactLabel::Kind, FactValue::text("image/jpeg"))
        .with(FactLabel::Dimensions, FactValue::text("2 × 2"))
        .with(FactLabel::Modified, FactValue::text("PDF's own"))
        .with_fact(
            Fact::new(FactLabel::Modified, FactValue::text("PDF's own"))
                .in_group(FactGroup::Document),
        );
    let file = Facts::empty()
        .with(FactLabel::Kind, FactValue::text("JPEG image"))
        .with(FactLabel::Modified, FactValue::text("the file's"));
    let merged = family.then(file);
    assert_eq!(
        merged.value_in(FactGroup::General, FactLabel::Kind),
        Some(&FactValue::text("JPEG image"))
    );
    assert_eq!(
        merged.value_in(FactGroup::Document, FactLabel::Modified),
        Some(&FactValue::text("PDF's own"))
    );
    assert_eq!(
        merged.value_in(FactGroup::General, FactLabel::Modified),
        Some(&FactValue::text("the file's"))
    );
    assert_eq!(merged.rows().len(), 4);
}

#[test]
fn a_section_can_be_withheld() {
    let facts = Facts::empty()
        .with(FactLabel::Coordinates, FactValue::text("somewhere"))
        .with(FactLabel::Camera, FactValue::text("Canon"))
        .without(FactGroup::Location);
    assert_eq!(facts.rows().len(), 1);
    assert_eq!(facts.value(FactLabel::Coordinates), None);
}

#[test]
fn a_needs_row_ends_the_general_section() {
    let facts = Facts::empty()
        .with(
            FactLabel::Needs,
            FactValue::text("anyview-heif (to show it)"),
        )
        .with(FactLabel::Kind, FactValue::text("HEIC image"))
        .with(FactLabel::Size, FactValue::text("1 MB"))
        .with(FactLabel::Camera, FactValue::text("Canon"))
        .with(FactLabel::Modified, FactValue::text("today"));
    let sections = facts.sections();
    let (group, rows) = sections.last().unwrap();
    assert_eq!(*group, FactGroup::General);
    let labels: Vec<_> = rows.iter().map(|row| row.label).collect();
    assert_eq!(
        labels,
        [
            FactLabel::Kind,
            FactLabel::Size,
            FactLabel::Modified,
            FactLabel::Needs
        ]
    );
}

#[test]
fn value_of_modified_is_the_files_own_date_not_the_documents() {
    let facts = Facts::empty()
        .with_fact(
            Fact::new(FactLabel::Modified, FactValue::text("PDF's own"))
                .in_group(FactGroup::Document),
        )
        .with(FactLabel::Modified, FactValue::text("the file's"));
    assert_eq!(
        facts.value(FactLabel::Modified),
        Some(&FactValue::text("the file's"))
    );
    let only_document = Facts::empty().with_fact(
        Fact::new(FactLabel::Modified, FactValue::text("PDF's own")).in_group(FactGroup::Document),
    );
    assert_eq!(
        only_document.value(FactLabel::Modified),
        Some(&FactValue::text("PDF's own"))
    );
}
