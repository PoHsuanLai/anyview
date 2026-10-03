//! Table tests for every `ExportChoice`: defaults, kinds and extensions.

use super::*;
use crate::source::FilePath;
use crate::units::{
    Dpi, PageIndex, PageRange, PageSelection, Percent, Permille, PixelLen, Quality,
};
use ds_core::word::Word;
use std::fmt::Debug;

/// Every implementation honours the same contract: the sheet lists every kind, a kind's default
/// is that kind, and the default's extension is stable.
fn assert_contract<E: ExportChoice + Debug>()
where
    E::Kind: Debug,
{
    assert_eq!(E::kinds(), <E::Kind as Word>::ALL);
    for kind in E::kinds() {
        let default = E::default_for(*kind);
        assert_eq!(
            default.kind(),
            *kind,
            "default_for({kind:?}) is {default:?}"
        );
        assert_eq!(
            E::default_for(*kind),
            default,
            "{kind:?} defaults the same way twice"
        );
    }
}

#[test]
fn every_export_enum_honours_the_contract() {
    assert_contract::<RasterExport>();
    assert_contract::<PdfExport>();
    assert_contract::<MediaExport>();
    assert_contract::<TextExport>();
    assert_contract::<NoExport>();
}

#[test]
fn a_format_with_no_export_lists_no_kind() {
    assert!(NoExport::kinds().is_empty());
    assert_eq!(RasterExport::kinds().len(), 6);
    assert_eq!(PdfExport::kinds().len(), 4);
    assert_eq!(MediaExport::kinds().len(), 5);
    assert_eq!(TextExport::kinds().len(), 2);
}

fn jpeg() -> RasterTarget {
    RasterTarget::Jpeg(Quality::clamped(Percent(90)))
}

fn avif() -> RasterTarget {
    RasterTarget::Avif(Quality::clamped(Percent(65)))
}

#[test]
fn raster_defaults_keep_the_pixels_and_name_their_extension() {
    use RasterExportKind::*;
    let image = |target| RasterExport::Image(target, Resize::Original);
    let cases = [
        (Png, image(RasterTarget::Png), ExportExtension::Png),
        (Jpeg, image(jpeg()), ExportExtension::Jpg),
        (Webp, image(RasterTarget::Webp), ExportExtension::Webp),
        (Avif, image(avif()), ExportExtension::Avif),
        (Tiff, image(RasterTarget::Tiff), ExportExtension::Tiff),
        (Pdf, RasterExport::Pdf, ExportExtension::Pdf),
    ];
    for (kind, want, extension) in cases {
        let got = RasterExport::default_for(kind);
        assert_eq!(got, want, "{kind:?}");
        assert_eq!(got.extension(), extension, "{kind:?}");
    }
}

#[test]
fn a_raster_choice_keeps_its_options_and_reports_its_kind() {
    let resized = RasterExport::Image(
        RasterTarget::Jpeg(Quality::clamped(Percent(40))),
        Resize::LongEdge(PixelLen(1024)),
    );
    assert_eq!(resized.kind(), RasterExportKind::Jpeg);
    assert_eq!(resized.extension(), ExportExtension::Jpg);
    assert_ne!(resized, RasterExport::default_for(RasterExportKind::Jpeg));
    let scaled = RasterExport::Image(RasterTarget::Png, Resize::Scaled(Permille(500)));
    assert_eq!(scaled.kind(), RasterExportKind::Png);
}

#[test]
fn pdf_defaults_cover_every_page() {
    use PdfExportKind::*;
    let cases = [
        (
            Pdf,
            PdfExport::Pdf(PageSelection::All),
            ExportExtension::Pdf,
        ),
        (
            PageImages,
            PdfExport::PageImages(PageSelection::All, RasterTarget::Png, Dpi::SCREEN),
            ExportExtension::Png,
        ),
        (PlainText, PdfExport::PlainText, ExportExtension::Txt),
        (Markdown, PdfExport::Markdown, ExportExtension::Md),
    ];
    for (kind, want, extension) in cases {
        let got = PdfExport::default_for(kind);
        assert_eq!(got, want, "{kind:?}");
        assert_eq!(got.extension(), extension, "{kind:?}");
    }
}

#[test]
fn a_pdf_choice_with_a_range_and_an_encoding_reports_both() {
    let range = PageRange::new(PageIndex(1), PageIndex(3)).unwrap();
    let images = PdfExport::PageImages(PageSelection::Range(range), jpeg(), Dpi::PRINT);
    assert_eq!(images.kind(), PdfExportKind::PageImages);
    assert_eq!(images.extension(), ExportExtension::Jpg);
    assert_ne!(images, PdfExport::default_for(PdfExportKind::PageImages));
}

#[test]
fn media_and_text_defaults() {
    const MEDIA: &[(MediaExportKind, ExportExtension)] = &[
        (MediaExportKind::FramePng, ExportExtension::Png),
        (MediaExportKind::FrameJpeg, ExportExtension::Jpg),
        (MediaExportKind::FrameWebp, ExportExtension::Webp),
        (MediaExportKind::FrameAvif, ExportExtension::Avif),
        (MediaExportKind::FrameTiff, ExportExtension::Tiff),
    ];
    for (kind, extension) in MEDIA {
        assert_eq!(
            MediaExport::default_for(*kind).extension(),
            *extension,
            "{kind:?}"
        );
    }
    assert_eq!(
        MediaExport::default_for(MediaExportKind::FrameJpeg),
        MediaExport::CurrentFrame(jpeg())
    );
    const TEXT: &[(TextExportKind, ExportExtension)] = &[
        (TextExportKind::Pdf, ExportExtension::Pdf),
        (TextExportKind::PlainText, ExportExtension::Txt),
    ];
    for (kind, extension) in TEXT {
        assert_eq!(
            TextExport::default_for(*kind).extension(),
            *extension,
            "{kind:?}"
        );
    }
    assert_eq!(
        TextExport::default_for(TextExportKind::Pdf),
        TextExport::Pdf(PrintLayout {
            paper: PaperSize::A4,
            orientation: Orientation::Portrait
        })
    );
}

#[test]
fn extensions_are_written_without_a_dot() {
    const CASES: &[(ExportExtension, &str)] = &[
        (ExportExtension::Png, "png"),
        (ExportExtension::Jpg, "jpg"),
        (ExportExtension::Webp, "webp"),
        (ExportExtension::Avif, "avif"),
        (ExportExtension::Tiff, "tiff"),
        (ExportExtension::Pdf, "pdf"),
        (ExportExtension::Txt, "txt"),
        (ExportExtension::Md, "md"),
    ];
    assert_eq!(CASES.len(), ExportExtension::ALL.len());
    for (extension, slug) in CASES {
        assert_eq!(extension.slug(), *slug);
        assert_eq!(ExportExtension::parse(slug), Some(*extension));
    }
}

#[test]
fn a_job_describes_its_payload_as_data() {
    let file = FilePath::new("/photos/cat.png").unwrap();
    let job = ExportJob::EncodeRaster {
        pixels: PixelSource::Image {
            file: file.clone(),
            resize: Resize::Original,
        },
        target: RasterTarget::Webp,
        keep: MetadataCarry::Keep,
    };
    assert_eq!(job.clone(), job);
    let other = ExportJob::EncodeRaster {
        pixels: PixelSource::Image {
            file,
            resize: Resize::Original,
        },
        target: RasterTarget::Webp,
        keep: MetadataCarry::Drop,
    };
    assert_ne!(job, other);
}
