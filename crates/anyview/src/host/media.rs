//! The media half of the desktop's tasks: playing a file with no window, and writing a cut, the
//! audio or the frame on screen beside a recording.

use super::outcome::Outcome;
use crate::media::{ExportEnd, ExportTool, Exports, MediaHub, WriteRoute};
use anyview_core::{
    AudioTarget, ExportJob, FileHead, FileName, FilePath, FormatDetail, FormatKind, MediaExport,
    RasterTarget, Resume, SniffStep, StreamPick, Subtitles, sniff,
};
use anyview_export::free_beside;
use anyview_media::{ExportRequest, MediaError, NameHints, ShotContent};
use anyview_plugin::Subject;
use anyview_ui::Probed;
use ds::prelude::Word;
use std::path::PathBuf;
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
    match media
        .hub
        .play_in_background(&probed.source, &probed.sniffed, resume)
    {
        Ok(()) => Outcome::Handed,
        Err(error) => Outcome::Failed(format!("cannot play the file: {error}")),
    }
}

/// Write `choice` for the recording `file` beside it.
pub(super) async fn export(media: &Media, file: &Probed, choice: MediaExport) -> Outcome {
    let jobs = anyview_media::plan_export(file.source.path(), choice);
    let mut outcome = Outcome::Nothing("there is nothing to write");
    for job in jobs {
        outcome = run(media, file, job).await;
        if matches!(outcome, Outcome::Failed(_)) {
            break;
        }
    }
    outcome
}

async fn run(media: &Media, file: &Probed, job: ExportJob) -> Outcome {
    match job {
        ExportJob::Transcode { .. } => transcode(media, file, job).await,
        ExportJob::MpvScreenshot { target, subtitles } => {
            frame(media, file.source.path(), target, subtitles).await
        }
        ExportJob::EncodeRaster { .. }
        | ExportJob::WritePdf { .. }
        | ExportJob::PrintToPdf { .. }
        | ExportJob::WriteText { .. } => Outcome::Nothing("not a recording's export"),
    }
}

/// A cut or a track, written by the FFmpeg plugin on a pool worker, next to the source under a
/// free name. Without that plugin the answer says which package adds it.
async fn transcode(media: &Media, file: &Probed, job: ExportJob) -> Outcome {
    let source = file.source.path();
    let subject = Subject {
        kind: file.sniffed.kind(),
        mime: Some(file.sniffed.mime()),
    };
    let tool = match media.hub.plugins().writer(&subject) {
        WriteRoute::Ready(tool) => tool,
        WriteRoute::Missing(missing) => {
            let error = MediaError::WriterMissing(missing.package.name());
            return Outcome::Failed(format!("cannot write the export: {error}"));
        }
        WriteRoute::Unserved => return Outcome::Failed("nothing writes this export".to_owned()),
    };
    let hints = {
        let (tool, file, job) = (Arc::clone(&tool), file.clone(), job.clone());
        match tokio::task::spawn_blocking(move || hints_of(&tool, &file, &job)).await {
            Ok(hints) => hints,
            Err(error) => return Outcome::Failed(format!("a task panicked: {error}")),
        }
    };
    let to = match anyview_media::output_path(source, &job, &hints, |path| !path.exists()) {
        Ok(to) => to,
        Err(error) => return Outcome::Failed(format!("cannot name the export: {error}")),
    };
    let request = ExportRequest {
        job,
        to,
        progress: Arc::new(|_| {}),
    };
    match media.exports.submit(tool, request).ended().await {
        ExportEnd::Written(_) => Outcome::Done,
        ExportEnd::Failed(error) => Outcome::Failed(format!("cannot write the export: {error}")),
        ExportEnd::Panicked(message) => Outcome::Failed(format!("the export panicked: {message}")),
        ExportEnd::Skipped => Outcome::Nothing("the export was stopped before it began"),
    }
}

/// What the name of the file depends on: the container the recording is in, and for a copy of
/// its sound the codec the track has, which the plugin says (a plugin that does not say leaves
/// the name to the container).
fn hints_of(tool: &ExportTool, file: &Probed, job: &ExportJob) -> NameHints {
    let container = if let FormatDetail::Media(container) = file.sniffed.detail() {
        Some(*container)
    } else {
        None
    };
    let copies_sound = matches!(
        job,
        ExportJob::Transcode {
            streams: StreamPick::AudioOnly,
            audio: AudioTarget::Copy,
            ..
        }
    );
    let audio_codec = copies_sound
        .then(|| tool.runner.probe(&tool.plugin, file.source.path()).ok())
        .flatten()
        .and_then(|rows| {
            let label = if file.sniffed.kind() == FormatKind::Video {
                "audio-codec"
            } else {
                "codec"
            };
            rows.into_iter()
                .find(|row| row.label == label)
                .map(|row| row.value)
        });
    NameHints {
        container,
        audio_codec,
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

#[cfg(test)]
mod tests {
    use super::*;

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
