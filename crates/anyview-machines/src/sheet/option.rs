//! The options of an export and how the sheet's machine applies one to a draft. Every option
//! checks itself against the draft and the open file: one that means nothing for this draft is
//! dropped, one outside its limits is brought inside.

use super::draft::ExportDraft;
use super::facts::ExportFacts;
use anyview_core::{
    AudioTarget, Bitrate, Dpi, MediaExport, MetadataCarry, Orientation, PageIndex, PageRange,
    PageSelection, PaperSize, PdfExport, Percent, Permille, PixelLen, PrintLayout, Quality,
    RasterExport, RasterExportKind, RasterTarget, Resize, TextExport, TimeRange,
};

/// The longest side an exported picture may be asked for, in pixels.
pub const MAX_LONG_EDGE: PixelLen = PixelLen(16_384);

/// Which pages the sheet's page choice stands for. It is the sheet's own state because a range
/// that happens to be one page is still a range the person is editing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PageSpan {
    /// Every page.
    #[default]
    All,
    /// The page the reader is on.
    Current,
    /// A run of pages, set with its first and last.
    Between,
}

/// Which part of a recording a trim keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrimSpan {
    /// The whole recording.
    Whole,
    /// What lies between the marks.
    Marks,
}

/// How a picture's size is said in the dialog: the sizes on offer, and the long edge a stepper sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizePick {
    /// As it is.
    Original,
    /// Half each way.
    Half,
    /// A quarter each way.
    Quarter,
    /// A long edge in pixels.
    LongEdge,
}

impl SizePick {
    /// The scale of "50%", in thousandths.
    const HALF: Permille = Permille(500);
    /// The scale of "25%", in thousandths.
    const QUARTER: Permille = Permille(250);

    /// The segment `resize` is, or `None` for a scale that is neither half nor a quarter: no
    /// segment is lit for a size the person did not pick from them.
    pub fn of(resize: Resize) -> Option<SizePick> {
        match resize {
            Resize::Original => Some(SizePick::Original),
            Resize::Scaled(scale) if scale == SizePick::HALF => Some(SizePick::Half),
            Resize::Scaled(scale) if scale == SizePick::QUARTER => Some(SizePick::Quarter),
            Resize::Scaled(_) => None,
            Resize::LongEdge(_) => Some(SizePick::LongEdge),
        }
    }

    /// The resize this segment asks for; `long` is the edge a long-edge pick starts on.
    pub fn resize(self, long: PixelLen) -> Resize {
        match self {
            SizePick::Original => Resize::Original,
            SizePick::Half => Resize::Scaled(SizePick::HALF),
            SizePick::Quarter => Resize::Scaled(SizePick::QUARTER),
            SizePick::LongEdge => Resize::LongEdge(long),
        }
    }
}

/// One option of an export changing. Page numbers count from one, as the person reads them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportOption {
    /// The quality of a JPEG or an AVIF.
    Quality(Percent),
    /// How a picture is resized.
    Resize(Resize),
    /// How much of a picture's metadata it keeps.
    Metadata(MetadataCarry),
    /// Which pages: all, the current one, or a range.
    Pages(PageSpan),
    /// The first page of a range, from 1.
    PagesFrom(u32),
    /// The last page of a range, from 1.
    PagesTo(u32),
    /// The format of the page images of a PDF.
    ImageFormat(RasterExportKind),
    /// The resolution of the page images of a PDF.
    Resolution(Dpi),
    /// The paper a text is printed on.
    Paper(PaperSize),
    /// Which way up the paper is.
    Orientation(Orientation),
    /// The part of a recording a trim keeps.
    Trim(TrimSpan),
    /// The bitrate of lossy audio.
    Bitrate(Bitrate),
}

/// The target with its quality set, when it has one.
fn requalified(target: RasterTarget, percent: Percent) -> RasterTarget {
    let quality = Quality::clamped(percent);
    match target {
        RasterTarget::Jpeg(_) => RasterTarget::Jpeg(quality),
        RasterTarget::Avif(_) => RasterTarget::Avif(quality),
        RasterTarget::Png | RasterTarget::Webp | RasterTarget::Tiff => target,
    }
}

/// The resize brought inside what can be made.
fn bounded(resize: Resize) -> Resize {
    match resize {
        Resize::Original => Resize::Original,
        Resize::Scaled(permille) => Resize::Scaled(Permille(permille.0.clamp(1, 1000))),
        Resize::LongEdge(edge) => Resize::LongEdge(PixelLen(edge.0.clamp(1, MAX_LONG_EDGE.0))),
    }
}

/// The encoding the format list's row for `kind` stands for, or `None` for the one that is not an
/// image. A lossy format picked while another lossy one was set keeps that one's quality: the
/// person who chose 70% did not choose to start again.
pub(super) fn target_of(kind: RasterExportKind, before: RasterTarget) -> Option<RasterTarget> {
    let target = match kind {
        RasterExportKind::Png => RasterTarget::Png,
        RasterExportKind::Jpeg => RasterTarget::default_jpeg(),
        RasterExportKind::Webp => RasterTarget::Webp,
        RasterExportKind::Avif => RasterTarget::default_avif(),
        RasterExportKind::Tiff => RasterTarget::Tiff,
        RasterExportKind::Pdf => return None,
    };
    Some(requality(target, before))
}

/// `target` with the quality `was` had, when both have one.
pub(super) fn requality(target: RasterTarget, was: RasterTarget) -> RasterTarget {
    match quality_of(was) {
        Some(percent) => requalified(target, percent),
        None => target,
    }
}

/// The pages `span` stands for in a document of the open file's length, from the range before.
fn selection(span: PageSpan, before: PageSelection, facts: &ExportFacts) -> PageSelection {
    let Some(count) = facts.several_pages() else {
        return PageSelection::All;
    };
    match (span, before) {
        (PageSpan::All, _) => PageSelection::All,
        (PageSpan::Current, _) => PageSelection::Range(PageRange::single(count.clamp(facts.page))),
        (PageSpan::Between, PageSelection::Range(range)) => PageSelection::Range(
            range
                .within(count)
                .unwrap_or(PageRange::single(count.last())),
        ),
        (PageSpan::Between, PageSelection::All) => PageSelection::Range(
            PageRange::new(PageIndex(0), count.last()).unwrap_or(PageRange::single(PageIndex(0))),
        ),
    }
}

/// The range with its first or last page moved to `page` (from 1), the other end making way so the
/// range is never reversed.
fn moved(before: PageSelection, facts: &ExportFacts, page: u32, first: bool) -> PageSelection {
    let (Some(count), PageSelection::Range(range)) = (facts.several_pages(), before) else {
        return before;
    };
    let at = count.clamp(PageIndex(page.saturating_sub(1)));
    let (from, to) = if first {
        (at, range.last().max(at))
    } else {
        (range.first().min(at), at)
    };
    PageRange::new(from, to).map_or(before, PageSelection::Range)
}

/// One control of the export sheet's options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExportControl {
    /// The quality slider.
    Quality,
    /// The picture's size.
    Size,
    /// How much metadata is kept: all, all but the location, or none.
    Metadata,
    /// All, the current page or a range.
    Pages,
    /// The first and last page of a range.
    PageRange,
    /// The format of the page images.
    ImageFormat,
    /// The resolution of the page images.
    Resolution,
    /// The paper.
    Paper,
    /// Which way up the paper is.
    Orientation,
    /// The whole recording or what lies between the marks.
    Range,
    /// The bitrate.
    Bitrate,
}

/// Whether `target` has a quality.
fn lossy(target: RasterTarget) -> bool {
    match target {
        RasterTarget::Jpeg(_) | RasterTarget::Avif(_) => true,
        RasterTarget::Png | RasterTarget::Webp | RasterTarget::Tiff => false,
    }
}

/// The quality of `target`, when it has one.
pub fn quality_of(target: RasterTarget) -> Option<Percent> {
    match target {
        RasterTarget::Jpeg(quality) | RasterTarget::Avif(quality) => Some(quality.percent()),
        RasterTarget::Png | RasterTarget::Webp | RasterTarget::Tiff => None,
    }
}

/// The format list's row that stands for `target`.
pub fn format_of(target: RasterTarget) -> RasterExportKind {
    match target {
        RasterTarget::Png => RasterExportKind::Png,
        RasterTarget::Jpeg(_) => RasterExportKind::Jpeg,
        RasterTarget::Webp => RasterExportKind::Webp,
        RasterTarget::Avif(_) => RasterExportKind::Avif,
        RasterTarget::Tiff => RasterExportKind::Tiff,
    }
}

impl ExportDraft {
    /// The options the sheet shows for this draft, top to bottom. None for a format with nothing
    /// to choose.
    pub fn controls(self, facts: &ExportFacts, span: PageSpan) -> Vec<ExportControl> {
        let pages = |controls: &mut Vec<ExportControl>| {
            if facts.several_pages().is_some() {
                controls.push(ExportControl::Pages);
                if span == PageSpan::Between {
                    controls.push(ExportControl::PageRange);
                }
            }
        };
        let mut controls = Vec::new();
        match self {
            ExportDraft::Raster(RasterExport::Image(target, ..)) => {
                if lossy(target) {
                    controls.push(ExportControl::Quality);
                }
                controls.push(ExportControl::Size);
                controls.push(ExportControl::Metadata);
            }
            ExportDraft::Pdf(PdfExport::Pdf(_)) => pages(&mut controls),
            ExportDraft::Pdf(PdfExport::PageImages(_, target, _)) => {
                pages(&mut controls);
                controls.push(ExportControl::ImageFormat);
                if lossy(target) {
                    controls.push(ExportControl::Quality);
                }
                controls.push(ExportControl::Resolution);
            }
            ExportDraft::Text(TextExport::Pdf(_)) => {
                controls.push(ExportControl::Paper);
                controls.push(ExportControl::Orientation);
            }
            ExportDraft::Media(MediaExport::CurrentFrame(target)) => {
                if lossy(target) {
                    controls.push(ExportControl::Quality);
                }
            }
            ExportDraft::Media(MediaExport::Trim(_)) => {
                if facts.marks.is_some() {
                    controls.push(ExportControl::Range);
                }
            }
            ExportDraft::Media(MediaExport::AudioOnly(
                AudioTarget::M4a(_) | AudioTarget::Mp3(_) | AudioTarget::Opus(_),
            )) => controls.push(ExportControl::Bitrate),
            ExportDraft::Raster(RasterExport::Pdf)
            | ExportDraft::Pdf(PdfExport::PlainText | PdfExport::Markdown)
            | ExportDraft::Text(TextExport::PlainText)
            | ExportDraft::Media(MediaExport::AudioOnly(
                AudioTarget::Copy | AudioTarget::Flac | AudioTarget::Wav,
            )) => {}
        }
        controls
    }

    /// The draft as the sheet first shows it for `facts`: a trim starts on the marks when there
    /// are some.
    pub fn seeded(self, facts: &ExportFacts) -> ExportDraft {
        match (self, facts.marks) {
            (ExportDraft::Media(MediaExport::Trim(_)), Some(marks)) => {
                ExportDraft::Media(MediaExport::Trim(marks))
            }
            _ => self,
        }
    }

    /// The draft and page choice after `option` changed. An option that means nothing for this
    /// draft changes nothing; one outside its limits lands on the nearest value inside them.
    pub fn tuned(
        self,
        option: ExportOption,
        facts: &ExportFacts,
        span: PageSpan,
    ) -> (ExportDraft, PageSpan) {
        let kept = (self, span);
        match (self, option) {
            (
                ExportDraft::Raster(RasterExport::Image(target, resize, keep)),
                ExportOption::Quality(percent),
            ) => (
                ExportDraft::Raster(RasterExport::Image(
                    requalified(target, percent),
                    resize,
                    keep,
                )),
                span,
            ),
            (
                ExportDraft::Raster(RasterExport::Image(target, _, keep)),
                ExportOption::Resize(resize),
            ) => (
                ExportDraft::Raster(RasterExport::Image(target, bounded(resize), keep)),
                span,
            ),
            (
                ExportDraft::Raster(RasterExport::Image(target, resize, _)),
                ExportOption::Metadata(keep),
            ) => (
                ExportDraft::Raster(RasterExport::Image(target, resize, keep)),
                span,
            ),
            (ExportDraft::Pdf(PdfExport::PageImages(pages, target, dpi)), option) => match option {
                ExportOption::Quality(percent) => (
                    ExportDraft::Pdf(PdfExport::PageImages(
                        pages,
                        requalified(target, percent),
                        dpi,
                    )),
                    span,
                ),
                ExportOption::ImageFormat(kind) => match target_of(kind, target) {
                    Some(target) => (
                        ExportDraft::Pdf(PdfExport::PageImages(pages, target, dpi)),
                        span,
                    ),
                    None => kept,
                },
                ExportOption::Resolution(dpi) => (
                    ExportDraft::Pdf(PdfExport::PageImages(pages, target, dpi)),
                    span,
                ),
                ExportOption::Pages(_) | ExportOption::PagesFrom(_) | ExportOption::PagesTo(_) => {
                    let (pages, span) = paged(pages, option, facts, span);
                    (
                        ExportDraft::Pdf(PdfExport::PageImages(pages, target, dpi)),
                        span,
                    )
                }
                ExportOption::Resize(_)
                | ExportOption::Metadata(_)
                | ExportOption::Paper(_)
                | ExportOption::Orientation(_)
                | ExportOption::Trim(_)
                | ExportOption::Bitrate(_) => kept,
            },
            (ExportDraft::Pdf(PdfExport::Pdf(pages)), option)
                if matches!(
                    option,
                    ExportOption::Pages(_) | ExportOption::PagesFrom(_) | ExportOption::PagesTo(_)
                ) =>
            {
                let (pages, span) = paged(pages, option, facts, span);
                (ExportDraft::Pdf(PdfExport::Pdf(pages)), span)
            }
            (ExportDraft::Text(TextExport::Pdf(layout)), ExportOption::Paper(paper)) => (
                ExportDraft::Text(TextExport::Pdf(PrintLayout { paper, ..layout })),
                span,
            ),
            (
                ExportDraft::Text(TextExport::Pdf(layout)),
                ExportOption::Orientation(orientation),
            ) => (
                ExportDraft::Text(TextExport::Pdf(PrintLayout {
                    orientation,
                    ..layout
                })),
                span,
            ),
            (
                ExportDraft::Media(MediaExport::CurrentFrame(target)),
                ExportOption::Quality(percent),
            ) => (
                ExportDraft::Media(MediaExport::CurrentFrame(requalified(target, percent))),
                span,
            ),
            (ExportDraft::Media(MediaExport::Trim(_)), ExportOption::Trim(part)) => {
                let range = match (part, facts.marks) {
                    (TrimSpan::Marks, Some(marks)) => marks,
                    (TrimSpan::Marks, None) | (TrimSpan::Whole, _) => TimeRange::WHOLE,
                };
                (ExportDraft::Media(MediaExport::Trim(range)), span)
            }
            (ExportDraft::Media(MediaExport::AudioOnly(target)), ExportOption::Bitrate(rate)) => {
                let target = match target {
                    AudioTarget::M4a(_) => AudioTarget::M4a(rate),
                    AudioTarget::Mp3(_) => AudioTarget::Mp3(rate),
                    AudioTarget::Opus(_) => AudioTarget::Opus(rate),
                    AudioTarget::Copy | AudioTarget::Flac | AudioTarget::Wav => target,
                };
                (ExportDraft::Media(MediaExport::AudioOnly(target)), span)
            }
            _ => kept,
        }
    }
}

/// The pages and page choice after a page option.
fn paged(
    before: PageSelection,
    option: ExportOption,
    facts: &ExportFacts,
    span: PageSpan,
) -> (PageSelection, PageSpan) {
    if facts.several_pages().is_none() {
        return (before, span);
    }
    match option {
        ExportOption::Pages(to) => (selection(to, before, facts), to),
        ExportOption::PagesFrom(page) if span == PageSpan::Between => {
            (moved(before, facts, page, true), span)
        }
        ExportOption::PagesTo(page) if span == PageSpan::Between => {
            (moved(before, facts, page, false), span)
        }
        ExportOption::PagesFrom(_)
        | ExportOption::PagesTo(_)
        | ExportOption::Quality(_)
        | ExportOption::Resize(_)
        | ExportOption::Metadata(_)
        | ExportOption::ImageFormat(_)
        | ExportOption::Resolution(_)
        | ExportOption::Paper(_)
        | ExportOption::Orientation(_)
        | ExportOption::Trim(_)
        | ExportOption::Bitrate(_) => (before, span),
    }
}
