//! Facts: the rows a pane lists about a file (dimensions, pages, duration…), each in a section
//! and at a tier, and the one-line summary the headline rows make.

mod file;
mod group;
mod kind_name;
mod label;
mod place;
mod summary;
mod value;
mod when;

pub use file::FileDetails;
pub use group::{FactGroup, Tier};
pub use kind_name::kind_name;
pub use label::FactLabel;
pub use place::Coordinate;
pub use value::FactValue;
pub use when::{FactTime, FactZone, LocalZone};

/// One row: a label and its value, in a section, at a tier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Fact {
    /// The section the row is listed in.
    pub group: FactGroup,
    /// What the row is about.
    pub label: FactLabel,
    /// What is true of it.
    pub value: FactValue,
    /// Whether the summary line carries it.
    pub tier: Tier,
}

impl Fact {
    /// The row `label` with `value`, in the label's own section and tier.
    #[must_use]
    pub fn new(label: FactLabel, value: FactValue) -> Self {
        Fact {
            group: label.group(),
            label,
            value,
            tier: label.tier(),
        }
    }

    /// This row, listed in `group` instead.
    #[must_use]
    pub fn in_group(self, group: FactGroup) -> Self {
        Fact { group, ..self }
    }

    /// This row, at `tier` instead.
    #[must_use]
    pub fn at_tier(self, tier: Tier) -> Self {
        Fact { tier, ..self }
    }
}

/// The rows a pane lists, in the order the producer gave them. Empty is a file with nothing to say.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Facts(Vec<Fact>);

impl Facts {
    /// No rows.
    #[must_use]
    pub fn empty() -> Self {
        Facts(Vec::new())
    }

    /// These rows plus one more at the end, in the label's own section and tier.
    #[must_use]
    pub fn with(self, label: FactLabel, value: FactValue) -> Self {
        self.with_fact(Fact::new(label, value))
    }

    /// These rows plus `fact` at the end.
    #[must_use]
    pub fn with_fact(mut self, fact: Fact) -> Self {
        self.0.push(fact);
        self
    }

    /// These rows, then `later`'s. A row of these that `later` also has (the same section and
    /// label) is dropped: the later, more exact producer wins.
    pub fn then(mut self, later: Facts) -> Self {
        self.0.retain(|own| {
            !later
                .0
                .iter()
                .any(|other| other.group == own.group && other.label == own.label)
        });
        self.0.extend(later.0);
        self
    }

    /// These rows without the ones listed in `group`.
    #[must_use]
    pub fn without(mut self, group: FactGroup) -> Self {
        self.0.retain(|row| row.group != group);
        self
    }

    /// The rows in order.
    pub fn rows(&self) -> &[Fact] {
        &self.0
    }

    /// The value of the row labelled `label` in the label's own section, else of the first row
    /// with that label. So `Modified` is the file's own date even when a document also lists the
    /// date it says it was modified.
    pub fn value(&self, label: FactLabel) -> Option<&FactValue> {
        self.value_in(label.group(), label).or_else(|| {
            self.0
                .iter()
                .find(|fact| fact.label == label)
                .map(|fact| &fact.value)
        })
    }

    /// The value of the first row labelled `label` in `group`.
    pub fn value_in(&self, group: FactGroup, label: FactLabel) -> Option<&FactValue> {
        self.0
            .iter()
            .find(|fact| fact.group == group && fact.label == label)
            .map(|fact| &fact.value)
    }

    /// The rows by section, sections in display order (General last) and each section's rows in
    /// the order they were given, except that a `Needs` row, which is about the viewer and not the
    /// file, ends the General section. A section with no rows is absent.
    pub fn sections(&self) -> Vec<(FactGroup, Vec<&Fact>)> {
        let mut sections: Vec<(FactGroup, Vec<&Fact>)> = Vec::new();
        for row in &self.0 {
            match sections.iter_mut().find(|(group, _)| *group == row.group) {
                Some((_, rows)) => rows.push(row),
                None => sections.push((row.group, vec![row])),
            }
        }
        sections.sort_by_key(|(group, _)| *group);
        for (_, rows) in &mut sections {
            rows.sort_by_key(|row| row.label == FactLabel::Needs);
        }
        sections
    }
}

#[cfg(test)]
mod tests;
