//! The Document section's rows for a PDF.

use super::{PdfInfo, Protection, Restriction, Tagging};
use anyview_core::{Fact, FactGroup, FactLabel, FactValue, Facts};

impl PdfInfo {
    /// The rows of the Document section, each only when the file has it: title, author, subject,
    /// keywords, created, modified, creator, producer, version, page size, pages, security,
    /// tagging, attachments and signatures.
    pub fn facts(&self) -> Facts {
        let text = |label: FactLabel, value: &Option<String>| {
            value
                .as_ref()
                .map(|value| Fact::new(label, FactValue::text(value.clone())))
        };
        let date = |label: FactLabel, when: Option<anyview_core::FactTime>| {
            when.map(|when| Fact::new(label, FactValue::date(when)).in_group(FactGroup::Document))
        };
        let rows = [
            text(FactLabel::Title, &self.title),
            text(FactLabel::Author, &self.author),
            text(FactLabel::Subject, &self.subject),
            text(FactLabel::Keywords, &self.keywords),
            date(FactLabel::Created, self.created),
            date(FactLabel::Modified, self.modified),
            text(FactLabel::Creator, &self.creator),
            text(FactLabel::Producer, &self.producer),
            self.version.map(|version| {
                Fact::new(
                    FactLabel::Version,
                    FactValue::version(&[u32::from(version.major), u32::from(version.minor)]),
                )
            }),
            Some(Fact::new(
                FactLabel::PageSize,
                FactValue::text(self.page_size.text()),
            )),
            Some(Fact::new(FactLabel::Pages, FactValue::pages(self.pages))),
            security(&self.protection)
                .map(|words| Fact::new(FactLabel::Security, FactValue::text(words))),
            (self.tagging == Tagging::Tagged)
                .then(|| Fact::new(FactLabel::Tagged, FactValue::text("Yes"))),
            count(self.attachments, "file", "files")
                .map(|words| Fact::new(FactLabel::Attachments, FactValue::text(words))),
            count(self.signatures, "signature", "signatures")
                .map(|words| Fact::new(FactLabel::Signatures, FactValue::text(words))),
        ];
        rows.into_iter()
            .flatten()
            .fold(Facts::empty(), Facts::with_fact)
    }
}

/// `1 file`, `3 files`; `None` for none.
fn count(n: usize, one: &str, many: &str) -> Option<String> {
    match n {
        0 => None,
        1 => Some(format!("1 {one}")),
        n => Some(format!("{n} {many}")),
    }
}

/// `Encrypted`, or `Encrypted (printing and copying not allowed)`; `None` for an open file.
fn security(protection: &Protection) -> Option<String> {
    let Protection::Encrypted(restrictions) = protection else {
        return None;
    };
    let words: Vec<&str> = restrictions
        .iter()
        .map(|restriction| match restriction {
            Restriction::Printing => "printing",
            Restriction::Copying => "copying",
            Restriction::Editing => "editing",
            Restriction::Commenting => "commenting",
        })
        .collect();
    Some(match words.as_slice() {
        [] => "Encrypted".to_owned(),
        [one] => format!("Encrypted ({one} not allowed)"),
        [rest @ .., last] => format!("Encrypted ({} and {last} not allowed)", rest.join(", ")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn security_names_what_was_taken_away() {
        let encrypted = |restrictions: &[Restriction]| Protection::Encrypted(restrictions.to_vec());
        assert_eq!(security(&Protection::Open), None);
        assert_eq!(security(&encrypted(&[])).as_deref(), Some("Encrypted"));
        assert_eq!(
            security(&encrypted(&[Restriction::Printing])).as_deref(),
            Some("Encrypted (printing not allowed)")
        );
        assert_eq!(
            security(&encrypted(&[Restriction::Printing, Restriction::Copying])).as_deref(),
            Some("Encrypted (printing and copying not allowed)")
        );
        assert_eq!(
            security(&encrypted(&[
                Restriction::Printing,
                Restriction::Copying,
                Restriction::Editing
            ]))
            .as_deref(),
            Some("Encrypted (printing, copying and editing not allowed)")
        );
    }
}
