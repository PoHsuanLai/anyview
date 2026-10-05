//! The stage and the file's view memory: what to remember of where the person is, and which input
//! puts a remembered place back.

use super::book::{BookIn, BookStage};
use super::family::StageFamily;
use super::model::{Stage, StageIn};
use super::pdf::{PdfIn, PdfStage};
use super::raster::{RasterIn, RasterStage};
use super::text::{TextIn, TextStage};
use anyview_core::{DocPoint, Resume, Zoom};

impl Stage {
    /// The family of stage this is; no stage is as a file with nothing to show but its facts.
    pub fn family(&self) -> StageFamily {
        match self {
            Stage::NoStage => StageFamily::PeekOnly,
            Stage::Raster(_) => StageFamily::Raster,
            Stage::Pdf(_) => StageFamily::Pdf,
            Stage::Media(_) => StageFamily::Media,
            Stage::Text(_) => StageFamily::Text,
            Stage::Table(_) => StageFamily::Table,
            Stage::Tree(_) => StageFamily::Tree,
            Stage::Book(_) => StageFamily::Book,
        }
    }

    /// Where the person is now, as the store keeps it. A stage with no place to keep (the media
    /// stage's position belongs to its player) is `Resume::Nothing`.
    pub fn resume(&self) -> Resume {
        match self {
            Stage::NoStage | Stage::Media(_) | Stage::Table(_) | Stage::Tree(_) => Resume::Nothing,
            Stage::Raster(RasterStage::Fitted { .. }) => Resume::Raster {
                zoom: Zoom::Fit,
                centre: DocPoint::default(),
            },
            Stage::Raster(
                RasterStage::Zoomed { zoom, centre, .. }
                | RasterStage::Panning { zoom, centre, .. },
            ) => Resume::Raster {
                zoom: *zoom,
                centre: *centre,
            },
            Stage::Pdf(
                PdfStage::Reading { view }
                | PdfStage::Finding { view, .. }
                | PdfStage::Jumping { view, .. },
            ) => Resume::Pdf {
                page: view.page,
                offset: view.offset,
                zoom: view.zoom,
            },
            Stage::Text(TextStage::Reading { place } | TextStage::Finding { place, .. }) => {
                Resume::Text { line: place.line }
            }
            Stage::Book(BookStage::Reading { section }) => Resume::Book { section: *section },
        }
    }

    /// The input that puts `resume` back, or `None` when it is not a place in this kind of stage
    /// (a place remembered for another kind of file, or nothing).
    pub fn restoring(&self, resume: &Resume) -> Option<StageIn> {
        match (self, resume) {
            // A table's sheet and a tree's open nodes are not kept: no place is put back.
            (Stage::Table(_) | Stage::Tree(_), _) => None,
            (Stage::Raster(_), Resume::Raster { zoom, centre }) => {
                Some(StageIn::Raster(RasterIn::Restore {
                    zoom: *zoom,
                    centre: *centre,
                }))
            }
            (Stage::Pdf(_), Resume::Pdf { .. }) => {
                Some(StageIn::Pdf(PdfIn::Restore(resume.clone())))
            }
            (Stage::Text(_), Resume::Text { .. }) => {
                Some(StageIn::Text(TextIn::Restore(resume.clone())))
            }
            (Stage::Book(_), Resume::Book { .. }) => {
                Some(StageIn::Book(BookIn::Restore(resume.clone())))
            }
            (
                Stage::NoStage | Stage::Media(_),
                Resume::Raster { .. }
                | Resume::Pdf { .. }
                | Resume::Media { .. }
                | Resume::Text { .. }
                | Resume::Book { .. }
                | Resume::Nothing,
            )
            | (
                Stage::Raster(_),
                Resume::Pdf { .. }
                | Resume::Media { .. }
                | Resume::Text { .. }
                | Resume::Book { .. }
                | Resume::Nothing,
            )
            | (
                Stage::Pdf(_),
                Resume::Raster { .. }
                | Resume::Media { .. }
                | Resume::Text { .. }
                | Resume::Book { .. }
                | Resume::Nothing,
            )
            | (
                Stage::Text(_),
                Resume::Raster { .. }
                | Resume::Pdf { .. }
                | Resume::Media { .. }
                | Resume::Book { .. }
                | Resume::Nothing,
            )
            | (
                Stage::Book(_),
                Resume::Raster { .. }
                | Resume::Pdf { .. }
                | Resume::Media { .. }
                | Resume::Text { .. }
                | Resume::Nothing,
            ) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stage::media::MediaStage;
    use crate::stage::pdf::PageView;
    use crate::stage::raster::Animation;
    use crate::stage::text::{TextPlace, TextView, Wrap};
    use anyview_core::{DocUnit, LineIndex, PageIndex, Permille, QuarterTurn, SectionIndex};
    use ds_core::machine::Machine;
    use ds_core::time::stamp::Stamp;

    const CENTRE: DocPoint = DocPoint {
        x: DocUnit(640),
        y: DocUnit(-64),
    };

    const fn zoomed(zoom: Zoom) -> Stage {
        Stage::Raster(RasterStage::Zoomed {
            turn: QuarterTurn::None,
            zoom,
            centre: CENTRE,
            anim: Animation::Still,
        })
    }

    const fn text_at(line: u32) -> Stage {
        Stage::Text(TextStage::Reading {
            place: TextPlace {
                line: LineIndex(line),
                wrap: Wrap::On,
                view: TextView::Source,
            },
        })
    }

    const fn book_at(section: u32) -> Stage {
        Stage::Book(crate::stage::BookStage::Reading {
            section: SectionIndex(section),
        })
    }

    const PDF_VIEW: PageView = PageView {
        page: PageIndex(4),
        offset: Permille(250),
        zoom: Zoom::Fit,
    };

    fn pdf() -> Stage {
        Stage::Pdf(PdfStage::Reading { view: PDF_VIEW })
    }

    #[test]
    fn a_stage_remembers_the_place_the_store_keeps_for_its_kind_of_file() {
        let cases: Vec<(&str, Stage, Resume)> = vec![
            ("nothing is shown", Stage::NoStage, Resume::Nothing),
            (
                "a fitted picture is remembered fitted",
                Stage::Raster(RasterStage::default()),
                Resume::Raster {
                    zoom: Zoom::Fit,
                    centre: DocPoint::default(),
                },
            ),
            (
                "a zoomed picture keeps its zoom and its centre",
                zoomed(Zoom::Actual),
                Resume::Raster {
                    zoom: Zoom::Actual,
                    centre: CENTRE,
                },
            ),
            (
                "a text keeps its first line",
                text_at(42),
                Resume::Text {
                    line: LineIndex(42),
                },
            ),
            (
                "a page keeps its page, how far down it and the zoom",
                pdf(),
                Resume::Pdf {
                    page: PageIndex(4),
                    offset: Permille(250),
                    zoom: Zoom::Fit,
                },
            ),
            (
                "a book keeps its section",
                book_at(6),
                Resume::Book {
                    section: SectionIndex(6),
                },
            ),
            (
                "a recording's place is its player's",
                Stage::Media(MediaStage::Opening),
                Resume::Nothing,
            ),
        ];
        for (name, stage, want) in cases {
            assert_eq!(stage.resume(), want, "{name}");
        }
    }

    #[test]
    fn a_remembered_place_is_put_back_only_into_the_kind_of_stage_it_was_made_for() {
        let raster = Resume::Raster {
            zoom: Zoom::Actual,
            centre: CENTRE,
        };
        let text = Resume::Text { line: LineIndex(7) };
        let book = Resume::Book {
            section: SectionIndex(2),
        };
        let page = Resume::Pdf {
            page: PageIndex(1),
            offset: Permille(0),
            zoom: Zoom::Fit,
        };
        let cases: Vec<(&str, Stage, Resume, Option<StageIn>)> = vec![
            (
                "a picture takes a zoom and a centre",
                Stage::Raster(RasterStage::default()),
                raster.clone(),
                Some(StageIn::Raster(RasterIn::Restore {
                    zoom: Zoom::Actual,
                    centre: CENTRE,
                })),
            ),
            (
                "a text takes a line",
                text_at(0),
                text.clone(),
                Some(StageIn::Text(TextIn::Restore(text.clone()))),
            ),
            (
                "a page takes a page",
                pdf(),
                page.clone(),
                Some(StageIn::Pdf(PdfIn::Restore(page.clone()))),
            ),
            (
                "a picture refuses a line",
                Stage::Raster(RasterStage::default()),
                text.clone(),
                None,
            ),
            ("a text refuses a zoom", text_at(0), raster.clone(), None),
            ("a page refuses nothing", pdf(), Resume::Nothing, None),
            (
                "a book takes a section",
                book_at(0),
                book.clone(),
                Some(StageIn::Book(BookIn::Restore(book.clone()))),
            ),
            ("a book refuses a line", book_at(0), text.clone(), None),
            ("a text refuses a section", text_at(0), book, None),
            ("no stage takes anything", Stage::NoStage, raster, None),
            (
                "a recording's is not the stage's",
                Stage::Media(MediaStage::Opening),
                text,
                None,
            ),
        ];
        for (name, stage, resume, want) in cases {
            assert_eq!(stage.restoring(&resume), want, "{name}");
        }
    }

    #[test]
    fn what_a_stage_remembers_is_what_it_comes_back_to() {
        let params = crate::StageParams {
            book: crate::BookParams {
                sections: anyview_core::SectionCount::new(5).unwrap(),
            },
            pdf: crate::PdfParams {
                pages: anyview_core::PageCount::new(10).unwrap(),
                ..crate::PdfParams::default()
            },
            ..crate::StageParams::default()
        };
        for (name, stage) in [
            ("a zoomed picture", zoomed(Zoom::Scale(Permille(2500)))),
            ("a text", text_at(311)),
            ("a page", pdf()),
            ("a book", book_at(2)),
        ] {
            let resume = stage.resume();
            let fresh = Stage::for_family(stage.family(), crate::TextViews::SourceOnly);
            let input = fresh.restoring(&resume).unwrap();
            let (back, _) = fresh.step(input, Stamp(0), &params, &());
            assert_eq!(back.resume(), resume, "{name}");
        }
    }
}
