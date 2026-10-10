//! Whether a file starts with its long lines wrapped: each kind has its own way until the person
//! has chosen another for it.

use super::model::Wrap;
use anyview_core::FormatKind;

/// The wrapping the person last chose for each kind of text. A kind never chosen for wraps as it
/// reads best: prose and Markdown wrap, code runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WrapChoices {
    plain: Option<Wrap>,
    markdown: Option<Wrap>,
    code: Option<Wrap>,
}

impl WrapChoices {
    /// How a file of `kind` starts.
    pub fn of(self, kind: FormatKind) -> Wrap {
        match kind {
            FormatKind::PlainText => self.plain.unwrap_or(Wrap::On),
            FormatKind::Markdown => self.markdown.unwrap_or(Wrap::On),
            FormatKind::Code => self.code.unwrap_or(Wrap::Off),
            FormatKind::Pdf
            | FormatKind::Raster
            | FormatKind::Vector
            | FormatKind::Video
            | FormatKind::Audio
            | FormatKind::Table
            | FormatKind::Tree
            | FormatKind::Font
            | FormatKind::Archive
            | FormatKind::Book
            | FormatKind::Office
            | FormatKind::Folder
            | FormatKind::Other => Wrap::On,
        }
    }

    /// The same, once the person has chosen `wrap` for `kind`.
    pub fn chose(self, kind: FormatKind, wrap: Wrap) -> WrapChoices {
        let chosen = Some(wrap);
        match kind {
            FormatKind::PlainText => WrapChoices {
                plain: chosen,
                ..self
            },
            FormatKind::Markdown => WrapChoices {
                markdown: chosen,
                ..self
            },
            FormatKind::Code => WrapChoices {
                code: chosen,
                ..self
            },
            FormatKind::Pdf
            | FormatKind::Raster
            | FormatKind::Vector
            | FormatKind::Video
            | FormatKind::Audio
            | FormatKind::Table
            | FormatKind::Tree
            | FormatKind::Font
            | FormatKind::Archive
            | FormatKind::Book
            | FormatKind::Office
            | FormatKind::Folder
            | FormatKind::Other => self,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kind_wraps_as_it_reads_best_until_the_person_chooses() {
        // name, kind, what it starts as, what is chosen, what it starts as then
        const CASES: &[(&str, FormatKind, Wrap, Wrap, Wrap)] = &[
            (
                "plain text wraps",
                FormatKind::PlainText,
                Wrap::On,
                Wrap::Off,
                Wrap::Off,
            ),
            (
                "Markdown wraps",
                FormatKind::Markdown,
                Wrap::On,
                Wrap::Off,
                Wrap::Off,
            ),
            (
                "code runs on",
                FormatKind::Code,
                Wrap::Off,
                Wrap::On,
                Wrap::On,
            ),
        ];
        for (name, kind, start, choice, then) in CASES {
            let none = WrapChoices::default();
            assert_eq!(none.of(*kind), *start, "{name}: at first");
            let chosen = none.chose(*kind, *choice);
            assert_eq!(chosen.of(*kind), *then, "{name}: after the choice");
        }
        let code_wrapped = WrapChoices::default().chose(FormatKind::Code, Wrap::On);
        assert_eq!(
            code_wrapped.of(FormatKind::PlainText),
            Wrap::On,
            "a choice is for its kind alone"
        );
        assert_eq!(code_wrapped.of(FormatKind::Markdown), Wrap::On);
    }
}
