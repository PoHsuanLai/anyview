//! What a person wrote on a document and has not saved.

use super::Mark;

/// A value written into a field of the form: the field's name, and the text, or the state name of
/// a check box or radio button (`Off` when clear).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldFill {
    /// The field's fully qualified name.
    pub name: String,
    /// What it now holds.
    pub value: String,
}

/// Everything a person wrote on a document and has not saved: the fields they filled, the marks
/// they made, and what they did to the marks the file already held.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct MarkupSave {
    /// The fields filled, one entry for each.
    pub fills: Vec<FieldFill>,
    /// The marks to write: those made, which have no [`Mark::existing`], and those the file held
    /// that were moved, recoloured or reworded, which have it. A mark the file held and the person
    /// left alone is not here.
    pub marks: Vec<Mark>,
    /// The marks the file held that were deleted, as they were in the file.
    pub removed: Vec<Mark>,
}

impl MarkupSave {
    /// Whether there is nothing to write.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fills.is_empty() && self.marks.is_empty() && self.removed.is_empty()
    }
}
