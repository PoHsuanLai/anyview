//! The one line a file is summarised by: `JPEG image · 4032 × 3024 · 3.2 MB`.

use super::{FactGroup, FactLabel, Facts, Tier};

/// The most parts a summary line has.
const PARTS: usize = 4;

impl Facts {
    /// The headline rows as one line, `a · b · c`, at most four parts: the kind first, then the
    /// section rows in the order given, then the general rows (the size) last. A part that
    /// repeats an earlier one is left out. `None` when no row is a headline.
    pub fn summary(&self) -> Option<String> {
        let headline = |row: &&super::Fact| row.tier == Tier::Headline;
        let kind = self
            .rows()
            .iter()
            .filter(headline)
            .filter(|row| row.label == FactLabel::Kind);
        let own = self
            .rows()
            .iter()
            .filter(headline)
            .filter(|row| row.group != FactGroup::General);
        let general = self
            .rows()
            .iter()
            .filter(headline)
            .filter(|row| row.group == FactGroup::General && row.label != FactLabel::Kind);
        let mut parts: Vec<&str> = Vec::new();
        for row in kind.chain(own).chain(general) {
            let text = row.value.as_str();
            if !text.is_empty() && !parts.contains(&text) && parts.len() < PARTS {
                parts.push(text);
            }
        }
        (!parts.is_empty()).then(|| parts.join(" · "))
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::source::ByteLen;

    #[test]
    fn the_summary_is_the_headline_rows_kind_first_size_last() {
        let facts = Facts::empty()
            .with(FactLabel::Dimensions, FactValue::text("4032 × 3024"))
            .with(FactLabel::Camera, FactValue::text("Canon EOS R5"))
            .with(FactLabel::Size, FactValue::size(ByteLen(3_214_880)))
            .with(FactLabel::Kind, FactValue::text("JPEG image"));
        assert_eq!(
            facts.summary().as_deref(),
            Some("JPEG image · 4032 × 3024 · 3.2 MB")
        );
    }

    #[test]
    fn the_summary_stops_at_four_parts_and_skips_repeats() {
        let facts = Facts::empty()
            .with(FactLabel::Kind, FactValue::text("PDF document"))
            .with(FactLabel::Pages, FactValue::text("12 pages"))
            .with(FactLabel::PageSize, FactValue::text("A4"))
            .with(FactLabel::Slides, FactValue::text("12 pages"))
            .with(FactLabel::Rows, FactValue::text("9 rows"))
            .with(FactLabel::Size, FactValue::text("1.1 MB"));
        assert_eq!(
            facts.summary().as_deref(),
            Some("PDF document · 12 pages · A4 · 9 rows")
        );
    }

    #[test]
    fn no_headline_means_no_summary() {
        assert_eq!(Facts::empty().summary(), None);
        let facts = Facts::empty().with(FactLabel::Camera, FactValue::text("Canon"));
        assert_eq!(facts.summary(), None);
    }
}
