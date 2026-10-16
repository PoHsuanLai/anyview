//! Applying an export option to a draft: which controls a draft shows, and how each option is checked against the draft and the open file.

use super::draft::ExportDraft;
use super::facts::ExportFacts;
use super::option::{ExportControl, ExportOption, MAX_LONG_EDGE, PageSpan, TrimSpan};
use anyview_core::{
    AudioTarget, MediaExport, PageIndex, PageRange, PageSelection, PdfExport, Percent, Permille,
    PixelLen, PrintLayout, Quality, RasterExport, RasterExportKind, RasterTarget, Resize,
    TextExport, TimeRange,
};

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
    #[must_use]
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
            ExportDraft::Raster(RasterExport::Adjusted(..) | RasterExport::Pdf)
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
    #[must_use]
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
    #[must_use]
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
