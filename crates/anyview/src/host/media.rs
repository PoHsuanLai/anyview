//! The media half of the desktop's tasks: playing a file with no window, and writing a cut, the
//! audio or the frame on screen beside a recording.

use super::outcome::Outcome;
use crate::media::{ExportEnd, Exports, MediaHub};
use anyview_core::{
    ExportJob, FileHead, FileName, FilePath, MediaExport, RasterTarget, Resume, SniffStep,
    Subtitles, sniff,
};
use anyview_media::{ExportRequest, ShotContent};
use anyview_ui::Probed;
use ds::prelude::Word;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// What the desktop plays and writes recordings with.
#[derive(Clone)]
pub struct Media {
    /// The players and the desktop's now-playing entry.
    pub hub: MediaHub,
    /// The pool's runner for transcodes.
    pub exports: Arc<Exports>,
    /// A folder for the files an export passes through.
    pub scratch: PathBuf,
}

impl std::fmt::Debug for Media {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Media").finish_non_exhaustive()
    }
}

/// Start the file playing with no window, from where it was left. Blocking.
pub(super) fn play_in_background(media: &Media, probed: &Probed) -> Outcome {
    let resume: &Resume = &probed.resume;
    match media.hub.play_in_background(probed.source.path(), resume) {
        Ok(()) => Outcome::Handed,
        Err(error) => Outcome::Failed(format!("cannot play the file: {error}")),
    }
}

/// Write `choice` for the recording `file` beside it.
pub(super) async fn export(media: &Media, file: &Probed, choice: MediaExport) -> Outcome {
    let jobs = anyview_media::plan_export(file.source.path(), choice);
    let mut outcome = Outcome::Nothing("there is nothing to write");
    for job in jobs {
        outcome = run(media, file.source.path(), job).await;
        if matches!(outcome, Outcome::Failed(_)) {
            break;
        }
    }
    outcome
}

async fn run(media: &Media, source: &FilePath, job: ExportJob) -> Outcome {
    match job {
        ExportJob::Transcode { .. } => transcode(media, source, job).await,
        ExportJob::MpvScreenshot { target, subtitles } => {
            frame(media, source, target, subtitles).await
        }
        ExportJob::EncodeRaster { .. }
        | ExportJob::WritePdf { .. }
        | ExportJob::PrintToPdf { .. }
        | ExportJob::WriteText { .. } => Outcome::Nothing("not a recording's export"),
    }
}

/// A cut or a track, written by a pool worker next to the source under a free name.
async fn transcode(media: &Media, source: &FilePath, job: ExportJob) -> Outcome {
    let reading = source.clone();
    let probed = tokio::task::spawn_blocking(move || anyview_media::probe(reading.as_path())).await;
    let probe = match probed {
        Ok(Ok(probe)) => probe,
        Ok(Err(error)) => return Outcome::Failed(format!("cannot read the recording: {error}")),
        Err(error) => return Outcome::Failed(format!("a task panicked: {error}")),
    };
    let to = match anyview_media::output_path(source, &job, &probe, |path| !path.exists()) {
        Ok(to) => to,
        Err(error) => return Outcome::Failed(format!("cannot name the export: {error}")),
    };
    let request = ExportRequest {
        job,
        to,
        progress: Arc::new(|_| {}),
    };
    match media.exports.submit(request).ended().await {
        ExportEnd::Written(_) => Outcome::Done,
        ExportEnd::Failed(error) => Outcome::Failed(format!("cannot write the export: {error}")),
        ExportEnd::Panicked(message) => Outcome::Failed(format!("the export panicked: {message}")),
        ExportEnd::Skipped => Outcome::Nothing("the export was stopped before it began"),
    }
}

/// The frame on screen, at the picture's own resolution: the player writes a PNG and the image
/// crate encodes it as `target` when that is another format.
async fn frame(
    media: &Media,
    source: &FilePath,
    target: RasterTarget,
    subtitles: Subtitles,
) -> Outcome {
    let content = match subtitles {
        Subtitles::Burn => ShotContent::Subtitles,
        Subtitles::Omit => ShotContent::Video,
    };
    let extension = target.extension().slug();
    let Some(final_path) = free_beside(source.as_path(), " frame", extension) else {
        return Outcome::Failed("cannot find a free name for the frame".to_owned());
    };
    let png = match target {
        RasterTarget::Png => final_path.clone(),
        RasterTarget::Jpeg(_) | RasterTarget::Webp | RasterTarget::Avif(_) | RasterTarget::Tiff => {
            media
                .scratch
                .join(format!("frame-{}.png", std::process::id()))
        }
    };
    let (Ok(shot), Ok(to)) = (FilePath::new(&png), FilePath::new(&final_path)) else {
        return Outcome::Failed("the frame's path is not absolute".to_owned());
    };
    if let Some(folder) = png.parent()
        && let Err(error) = std::fs::create_dir_all(folder)
    {
        return Outcome::Failed(format!("cannot make a folder for the frame: {error}"));
    }
    if let Err(reason) = media.hub.screenshot(source, shot.clone(), content).await {
        return Outcome::Failed(format!("cannot save the frame: {reason}"));
    }
    match target {
        RasterTarget::Png => Outcome::Done,
        RasterTarget::Jpeg(_) | RasterTarget::Webp | RasterTarget::Avif(_) | RasterTarget::Tiff => {
            let outcome = tokio::task::spawn_blocking(move || recode(&shot, &to, target)).await;
            outcome.unwrap_or_else(|error| Outcome::Failed(format!("a task panicked: {error}")))
        }
    }
}

/// The PNG at `from` written to `to` as `target`; the PNG is removed.
fn recode(from: &FilePath, to: &FilePath, target: RasterTarget) -> Outcome {
    let result = recoded(from, to, target);
    let _gone = std::fs::remove_file(from.as_path());
    match result {
        Ok(()) => Outcome::Done,
        Err(reason) => Outcome::Failed(format!("cannot encode the frame: {reason}")),
    }
}

fn recoded(from: &FilePath, to: &FilePath, target: RasterTarget) -> Result<(), String> {
    let bytes = std::fs::read(from.as_path()).map_err(|error| error.to_string())?;
    let name = FileName::new("frame.png").map_err(|error| error.to_string())?;
    let head = &bytes[..bytes.len().min(4096)];
    let SniffStep::Done(sniffed) = sniff(&FileHead::new(head), &name) else {
        return Err("the frame is not a picture".to_owned());
    };
    let picture = match anyview_image::decode_bytes(&bytes, &sniffed) {
        Ok(anyview_image::Decoded::Still(picture)) => picture,
        Ok(anyview_image::Decoded::Animated(_)) => {
            return Err("the frame is an animation".to_owned());
        }
        Err(error) => return Err(error.to_string()),
    };
    let encoded = anyview_image::encode(&picture, target).map_err(|error| error.to_string())?;
    std::fs::write(to.as_path(), encoded).map_err(|error| error.to_string())
}

/// `<stem><suffix>.<extension>` beside `file`, or with ` 2`, ` 3`, ... before the extension: the
/// first that is free.
fn free_beside(file: &Path, suffix: &str, extension: &str) -> Option<PathBuf> {
    let folder = file.parent()?;
    let stem = file.file_stem()?.to_string_lossy().into_owned();
    (1_u32..)
        .map(|n| match n {
            1 => format!("{stem}{suffix}.{extension}"),
            n => format!("{stem}{suffix} {n}.{extension}"),
        })
        .map(|name| folder.join(name))
        .take(10_000)
        .find(|candidate| !candidate.exists())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_beside_the_file_is_the_first_free_one() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("Holiday.mkv");
        let name = |text: &str| dir.path().join(text);
        assert_eq!(
            free_beside(&file, " frame", "png"),
            Some(name("Holiday frame.png"))
        );
        std::fs::write(name("Holiday frame.png"), "x").unwrap();
        assert_eq!(
            free_beside(&file, " frame", "png"),
            Some(name("Holiday frame 2.png")),
            "a taken name is never overwritten"
        );
        std::fs::write(name("Holiday frame 2.png"), "x").unwrap();
        assert_eq!(
            free_beside(&file, " frame", "png"),
            Some(name("Holiday frame 3.png"))
        );
        assert_eq!(
            free_beside(&file, " frame", "jpg"),
            Some(name("Holiday frame.jpg")),
            "another extension is another name"
        );
    }

    #[test]
    fn a_frame_is_recoded_to_the_format_asked_and_the_png_goes() {
        let dir = tempfile::tempdir().unwrap();
        let shot = dir.path().join("shot.png");
        // A 4 by 2 PNG: the encoder of the image crate writes it.
        let size = anyview_core::PixelSize {
            width: anyview_core::PixelLen(4),
            height: anyview_core::PixelLen(2),
        };
        let picture = anyview_image::Rgba8::new(size, vec![200; 4 * 2 * 4]).unwrap();
        std::fs::write(
            &shot,
            anyview_image::encode(&picture, RasterTarget::Png).unwrap(),
        )
        .unwrap();
        let (from, to) = (
            FilePath::new(&shot).unwrap(),
            FilePath::new(dir.path().join("out.tiff")).unwrap(),
        );
        assert_eq!(recode(&from, &to, RasterTarget::Tiff), Outcome::Done);
        let bytes = std::fs::read(to.as_path()).unwrap();
        assert!(
            bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*"),
            "a TIFF"
        );
        assert!(!shot.exists(), "the PNG the player wrote is removed");
    }

    #[test]
    fn a_frame_that_is_not_a_picture_is_a_failure_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        let shot = dir.path().join("shot.png");
        std::fs::write(&shot, b"not a picture").unwrap();
        let (from, to) = (
            FilePath::new(&shot).unwrap(),
            FilePath::new(dir.path().join("out.jpg")).unwrap(),
        );
        assert!(matches!(
            recode(&from, &to, RasterTarget::default_jpeg()),
            Outcome::Failed(_)
        ));
    }
}
