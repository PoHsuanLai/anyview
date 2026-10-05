//! A text choice as export jobs.

use anyview_core::{
    ExportJob, FilePath, FormatDetail, FormatKind, HtmlDoc, PrintLayout, Sniffed, TextExport,
    TextSource,
};

/// The jobs that write `choice` for the text file `file`: a PDF is its page laid out (a source
/// file is printed highlighted), plain text is the file as it is. A kind that has no text
/// document (a picture, a recording) has no jobs.
pub fn plan_export(file: &FilePath, sniffed: &Sniffed, choice: TextExport) -> Vec<ExportJob> {
    match choice {
        TextExport::Pdf(layout) => printed(file, sniffed, layout),
        TextExport::PlainText => {
            let is_text = matches!(
                sniffed.kind(),
                FormatKind::Markdown | FormatKind::Code | FormatKind::PlainText
            );
            if is_text {
                vec![ExportJob::WriteText {
                    text: TextSource::File(file.clone()),
                }]
            } else {
                Vec::new()
            }
        }
    }
}

/// The jobs that make the PDF a text file is printed from, on the default paper.
pub fn plan_print(file: &FilePath, sniffed: &Sniffed) -> Vec<ExportJob> {
    printed(file, sniffed, PrintLayout::default())
}

fn printed(file: &FilePath, sniffed: &Sniffed, layout: PrintLayout) -> Vec<ExportJob> {
    let html = match (sniffed.kind(), sniffed.detail()) {
        (FormatKind::Markdown, _) => HtmlDoc::Markdown(file.clone()),
        (FormatKind::Code, FormatDetail::Code(syntax)) => HtmlDoc::Code {
            file: file.clone(),
            syntax: syntax.clone(),
        },
        (FormatKind::Code | FormatKind::PlainText, _) => HtmlDoc::PlainText(file.clone()),
        _ => return Vec::new(),
    };
    vec![ExportJob::PrintToPdf { html, layout }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyview_core::{FileHead, FileName, SniffStep, SyntaxName, sniff};

    fn sniffed(name: &str, bytes: &[u8]) -> Sniffed {
        match sniff(&FileHead::new(bytes), &FileName::new(name).unwrap()) {
            SniffStep::Done(sniffed) => sniffed,
            SniffStep::LookInside(_) => panic!("{name} is not a zip"),
        }
    }

    #[test]
    fn each_kind_of_text_is_printed_as_its_own_page_and_written_as_it_is() {
        let file = FilePath::new("/docs/file").unwrap();
        let pdf = TextExport::Pdf(PrintLayout::default());
        let print = |html| {
            vec![ExportJob::PrintToPdf {
                html,
                layout: PrintLayout::default(),
            }]
        };
        let copy = vec![ExportJob::WriteText {
            text: TextSource::File(file.clone()),
        }];
        let cases = [
            (
                "markdown prints rendered",
                sniffed("a.md", b"# hi"),
                pdf,
                print(HtmlDoc::Markdown(file.clone())),
            ),
            (
                "code prints highlighted as its syntax",
                sniffed("a.rs", b"fn main() {}"),
                pdf,
                print(HtmlDoc::Code {
                    file: file.clone(),
                    syntax: SyntaxName::new("rust").unwrap(),
                }),
            ),
            (
                "plain text prints as it is",
                sniffed("a.txt", b"words"),
                pdf,
                print(HtmlDoc::PlainText(file.clone())),
            ),
            (
                "markdown is written as its text",
                sniffed("a.md", b"# hi"),
                TextExport::PlainText,
                copy.clone(),
            ),
            (
                "a picture has no text to write",
                sniffed("a.png", b"\x89PNG\r\n\x1a\n"),
                TextExport::PlainText,
                Vec::new(),
            ),
            (
                "a picture has no page to print",
                sniffed("a.png", b"\x89PNG\r\n\x1a\n"),
                pdf,
                Vec::new(),
            ),
        ];
        for (name, sniffed, choice, want) in cases {
            assert_eq!(plan_export(&file, &sniffed, choice), want, "{name}");
        }
    }
}
