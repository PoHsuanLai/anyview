//! The stage registry: the one exhaustive match over `FormatKind` for the full tier. Adding a kind
//! is a compile error here until it names the view that shows it, and every kind without its own
//! view is mapped to [`PeekOnlyStageView`], never left out. `anyview_core::stage_support` says the same
//! thing in `anyview-core`'s own table; a test holds the two equal.

use super::media::MediaStageView;
use super::pdf::PdfStageView;
use super::peek_only::PeekOnlyStageView;
use super::raster::RasterStageView;
use super::table::TableStageView;
use super::text::TextStageView;
use super::tree::TreeStageView;
use super::view::{LoadedDoc, StageView};
use crate::io::{OpenError, OpenLink};
use crate::{LoadFlow, StageFamily, Ticket};
use anyview_core::{Facts, FormatKind, Input, Sniffed, Source};
use anyview_store::general_facts as general_of;

/// Something done with the view that shows a kind, without naming it.
pub trait KindVisitor {
    /// What it makes.
    type Out;
    /// Do it for the view `S`.
    fn visit<S: StageView>(self) -> Self::Out;
}

/// Run `visitor` for the view that shows `kind`.
pub fn visit<V: KindVisitor>(kind: FormatKind, visitor: V) -> V::Out {
    match kind {
        FormatKind::Raster | FormatKind::Vector => visitor.visit::<RasterStageView>(),
        FormatKind::Markdown | FormatKind::Code | FormatKind::PlainText => {
            visitor.visit::<TextStageView>()
        }
        FormatKind::Table => visitor.visit::<TableStageView>(),
        FormatKind::Tree => visitor.visit::<TreeStageView>(),
        FormatKind::Book | FormatKind::Pdf => visitor.visit::<PdfStageView>(),
        FormatKind::Video | FormatKind::Audio => visitor.visit::<MediaStageView>(),
        FormatKind::Font
        | FormatKind::Archive
        | FormatKind::Office
        | FormatKind::Folder
        | FormatKind::Other => visitor.visit::<PeekOnlyStageView>(),
    }
}

struct FamilyOf;

impl KindVisitor for FamilyOf {
    type Out = StageFamily;

    fn visit<S: StageView>(self) -> StageFamily {
        S::FAMILY
    }
}

/// The family of stage that shows `kind`.
pub fn family_of(kind: FormatKind) -> StageFamily {
    visit(kind, FamilyOf)
}

/// The General section of the file `src`: what the file system says of it. Blocking.
fn general_facts(src: &Source, sniffed: &Sniffed) -> Facts {
    general_of(&Input::from(src), sniffed)
}

struct Opener<'a> {
    ticket: Ticket,
    src: &'a Source,
    sniffed: &'a Sniffed,
    link: &'a OpenLink,
}

impl KindVisitor for Opener<'_> {
    type Out = Result<LoadedDoc, OpenError>;

    fn visit<S: StageView>(self) -> Self::Out {
        let general = general_facts(self.src, self.sniffed);
        S::open(self.ticket, self.src, self.sniffed, self.link)
            .map(|doc| LoadedDoc::of::<S>(doc).describing(general))
    }
}

/// Open the file whose type `sniffed` established with the view that shows it. Blocking.
pub(crate) fn open_for(
    ticket: Ticket,
    src: &Source,
    sniffed: &Sniffed,
    link: &OpenLink,
) -> Result<LoadedDoc, OpenError> {
    visit(
        sniffed.kind(),
        Opener {
            ticket,
            src,
            sniffed,
            link,
        },
    )
}

struct FlowOf;

impl KindVisitor for FlowOf {
    type Out = LoadFlow;

    fn visit<S: StageView>(self) -> LoadFlow {
        S::FLOW
    }
}

/// How a file of `kind` is opened: with a cheap first frame beside the open, or only the open.
pub fn flow_of(kind: FormatKind) -> LoadFlow {
    visit(kind, FlowOf)
}

struct FirstFrame<'a> {
    ticket: Ticket,
    src: &'a Source,
    sniffed: &'a Sniffed,
    link: &'a OpenLink,
}

impl KindVisitor for FirstFrame<'_> {
    type Out = Result<Option<LoadedDoc>, OpenError>;

    fn visit<S: StageView>(self) -> Self::Out {
        let doc = S::first_frame(self.ticket, self.src, self.sniffed, self.link)?;
        let general = general_facts(self.src, self.sniffed);
        Ok(doc.map(|doc| LoadedDoc::of::<S>(doc).describing(general)))
    }
}

/// The cheap first frame of the file whose type `sniffed` established, or `None` when it has no
/// frame cheaper than opening it. Blocking.
pub(crate) fn peek_for(
    ticket: Ticket,
    src: &Source,
    sniffed: &Sniffed,
    link: &OpenLink,
) -> Result<Option<LoadedDoc>, OpenError> {
    visit(
        sniffed.kind(),
        FirstFrame {
            ticket,
            src,
            sniffed,
            link,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::{PeekOnlyDoc, PeekOnlyStageView};
    use crate::io::Readable;
    use anyview_core::{
        ByteLen, FactGroup, FactLabel, FactValue, FileHead, FileName, FilePath, FileStamp, ModTime,
        SniffStep, sniff,
    };

    /// A text file on disk, as the viewer is handed it.
    fn text_file(dir: &std::path::Path) -> (Source, Sniffed) {
        let path = dir.join("notes.txt");
        std::fs::write(&path, b"hello\n").unwrap();
        let stamp = FileStamp {
            len: ByteLen(6),
            modified: ModTime(0),
        };
        let name = FileName::new("notes.txt").unwrap();
        let SniffStep::Done(sniffed) = sniff(&FileHead::new(b"hello\n"), &name) else {
            panic!("a text file is not a zip");
        };
        (Source::new(FilePath::new(&path).unwrap(), stamp), sniffed)
    }

    #[test]
    fn a_files_general_section_comes_from_the_file_system() {
        let dir = tempfile::tempdir().unwrap();
        let (src, sniffed) = text_file(dir.path());
        let general = general_facts(&src, &sniffed);
        let value = |label| general.value(label).map(FactValue::as_str);
        assert_eq!(value(FactLabel::Kind), Some("Plain text"));
        assert_eq!(value(FactLabel::Size), Some("6 B"));
        assert_eq!(value(FactLabel::Where), Some(dir.path().to_str().unwrap()));
        assert!(value(FactLabel::Modified).is_some());
        assert!(value(FactLabel::Permissions).is_some());
        assert!(
            general
                .rows()
                .iter()
                .all(|row| row.group == FactGroup::General)
        );
    }

    #[test]
    fn the_info_tab_lists_the_family_rows_then_general_with_the_file_systems_kind_and_size() {
        let dir = tempfile::tempdir().unwrap();
        let (src, sniffed) = text_file(dir.path());
        let family = Facts::empty()
            .with(FactLabel::Kind, FactValue::text("text/plain"))
            .with(FactLabel::Lines, FactValue::text("1"))
            .with(FactLabel::Size, FactValue::text("stale"));
        let doc = LoadedDoc::of::<PeekOnlyStageView>(PeekOnlyDoc {
            name: "notes.txt".to_owned(),
            kind: sniffed.kind(),
            facts: family,
            thumbnail: None,
            listing: Vec::new(),
            readable: Readable::Yes,
        })
        .describing(general_facts(&src, &sniffed));
        let groups: Vec<FactGroup> = doc.facts().sections().iter().map(|(g, _)| *g).collect();
        assert_eq!(groups, [FactGroup::Text, FactGroup::General]);
        let facts = doc.facts();
        assert_eq!(
            facts.value(FactLabel::Kind).map(FactValue::as_str),
            Some("Plain text")
        );
        assert_eq!(
            facts.value(FactLabel::Size).map(FactValue::as_str),
            Some("6 B")
        );
        assert_eq!(
            facts.value(FactLabel::Lines).map(FactValue::as_str),
            Some("1")
        );
    }

    #[test]
    fn a_document_with_no_file_behind_it_has_no_general_section() {
        let doc = LoadedDoc::of::<PeekOnlyStageView>(PeekOnlyDoc {
            name: "attachment.txt".to_owned(),
            kind: FormatKind::PlainText,
            facts: Facts::empty().with(FactLabel::Lines, FactValue::text("1")),
            thumbnail: None,
            listing: Vec::new(),
            readable: Readable::Yes,
        });
        let groups: Vec<FactGroup> = doc.facts().sections().iter().map(|(g, _)| *g).collect();
        assert_eq!(groups, [FactGroup::Text]);
    }
}
