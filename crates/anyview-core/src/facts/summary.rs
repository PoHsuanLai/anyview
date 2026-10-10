//! The one line a file is summarised by under its name, as Finder's and Preview's info windows
//! have it: `JPEG image · 3.2 MB`. The rest of the facts are the details below it.

use super::{FactLabel, Facts};

impl Facts {
    /// The kind and the size, `kind · size`, with the size in its short form (`3.2 MB`, not the
    /// exact byte count beside it). `None` when the facts have neither.
    pub fn summary(&self) -> Option<String> {
        let value = |label: FactLabel| {
            self.rows()
                .iter()
                .find(|row| row.label == label)
                .map(|row| row.value.as_str())
                .filter(|text| !text.is_empty())
        };
        // An exact size reads `3.2 MB (3,214,880 bytes)`; the summary keeps the part before it.
        let size = value(FactLabel::Size).map(|text| text.split(" (").next().unwrap_or(text));
        let parts: Vec<&str> = value(FactLabel::Kind).into_iter().chain(size).collect();
        (!parts.is_empty()).then(|| parts.join(" · "))
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::source::ByteLen;

    #[test]
    fn the_summary_is_the_kind_and_the_short_size() {
        // name, facts, summary
        let cases: [(&str, Facts, Option<&str>); 4] = [
            (
                "a photo keeps its dimensions and camera for the details",
                Facts::empty()
                    .with(FactLabel::Dimensions, FactValue::text("4032 × 3024"))
                    .with(FactLabel::Camera, FactValue::text("Canon EOS R5"))
                    .with(FactLabel::Size, FactValue::size_exact(ByteLen(3_214_880)))
                    .with(FactLabel::Kind, FactValue::text("JPEG image")),
                Some("JPEG image · 3.2 MB"),
            ),
            (
                "a kind alone",
                Facts::empty()
                    .with(FactLabel::Kind, FactValue::text("PDF document"))
                    .with(FactLabel::Pages, FactValue::text("12 pages")),
                Some("PDF document"),
            ),
            (
                "a size alone",
                Facts::empty().with(FactLabel::Size, FactValue::size(ByteLen(621))),
                Some("621 B"),
            ),
            (
                "neither",
                Facts::empty().with(FactLabel::Camera, FactValue::text("Canon")),
                None,
            ),
        ];
        for (name, facts, summary) in cases {
            assert_eq!(facts.summary().as_deref(), summary, "{name}");
        }
    }
}
