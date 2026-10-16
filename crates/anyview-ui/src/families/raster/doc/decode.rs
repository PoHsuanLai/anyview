//! Decoding a picture's bytes, by the codec or by a plugin, into a document on the GPU.

use super::build::{doc_of, upload, upload_frames};
use super::types::{RasterDoc, RasterOpen};
use crate::io::{Need, OpenError, PluginPicture};
use crate::{EditCaution, EditOffer, Runs};
use anyview_core::{
    FactLabel, FactValue, Facts, FormatDetail, FormatKind, PixelArea, PixelLen, PixelSize,
    RasterFormat, RasterTarget, Sniffed, Source,
};
use anyview_fs::OnDisk;
use anyview_image::{Decoded, Fidelity, ImageError, Plays, decode_bytes, fidelity, file_bytes};
use std::sync::Arc;

/// The most pixels a plugin may be asked to decode: a 64 megapixel photograph, 256 MiB of RGBA.
const PLUGIN_AREA: PixelArea = PixelArea(64 * 1024 * 1024);
pub(super) fn decode_into(target: &RasterOpen) -> Result<RasterDoc, OpenError> {
    OpenError::admit_len(target.source.stamp().len)?;
    // The file is read once: the decoder, the facts and the edit offer share these bytes.
    let bytes = file_bytes(&target.source.on_disk())?;
    let mut doc = decoded(target, &bytes)?;
    // A picture with no pixels was made `Withheld` there, and one a plugin decoded `Copies`: the
    // headers of its file say nothing of what it can take.
    if doc.offer == EditOffer::Plain {
        doc.offer = offer_of(&target.source, &target.sniffed, &bytes, doc.frames);
    }
    Ok(doc)
}
/// What saving a turn or flip of the file costs, read from its headers. A file too large to read
/// whole for this is asked about nothing: the edit's own limits answer. A file with no writer is
/// changed all the same and saved as a copy; only a vector picture, and an animation that no
/// still copy could hold, are not changed.
fn offer_of(source: &Source, sniffed: &Sniffed, bytes: &[u8], frames: u32) -> EditOffer {
    const LIMIT: u64 = 512 * 1024 * 1024;
    if source.stamp().len.0 > LIMIT {
        return EditOffer::Plain;
    }
    match fidelity(bytes, sniffed) {
        Fidelity::Intact => EditOffer::Plain,
        Fidelity::Loses(loss) => EditOffer::Asks(EditCaution::Loses(loss.sentence())),
        Fidelity::Impossible if sniffed.kind() == FormatKind::Vector || frames > 1 => {
            EditOffer::Withheld
        }
        Fidelity::Impossible => EditOffer::Copies(copy_target(sniffed)),
    }
}
/// The format a copy of a picture with no writer is written in: JPEG for the formats photographs
/// come in, PNG for everything else (art, icons, flattened layers, high range pictures).
fn copy_target(sniffed: &Sniffed) -> RasterTarget {
    let photo = matches!(
        sniffed.detail(),
        FormatDetail::Raster(
            RasterFormat::Avif | RasterFormat::Jxl | RasterFormat::Heic | RasterFormat::Raw,
        )
    );
    if photo {
        RasterTarget::default_jpeg()
    } else {
        RasterTarget::Png
    }
}
fn decoded(target: &RasterOpen, bytes: &[u8]) -> Result<RasterDoc, OpenError> {
    if matches!(
        target.sniffed.detail(),
        FormatDetail::Raster(RasterFormat::Raw)
    ) {
        return raw_into(target, bytes);
    }
    match decode_bytes(bytes, &target.sniffed) {
        Ok(Decoded::Still(picture)) => {
            let held = upload(&target.texture, &picture)?;
            Ok(holding(
                doc_of(target, bytes, picture.size(), 1, None, Runs::Forever),
                held,
            ))
        }
        Ok(Decoded::HeldStill { picture, frames }) => {
            let held = upload(&target.texture, &picture)?;
            let mut doc = holding(
                doc_of(target, bytes, picture.size(), frames.0, None, Runs::Forever),
                held,
            );
            doc.facts = doc.facts.with(
                FactLabel::Frames,
                FactValue::text(format!(
                    "{} (too large to play; showing the first)",
                    frames.0
                )),
            );
            Ok(doc)
        }
        Ok(Decoded::Animated(animation)) => {
            let (strip, held) = upload_frames(&target.texture, &animation)?;
            let first = animation.frames.first().pixels.size();
            let count = u32::try_from(animation.frames.count().get()).unwrap_or(u32::MAX);
            let runs = match animation.plays {
                Plays::Forever => Runs::Forever,
                Plays::Times(times) => Runs::Times(times),
            };
            Ok(holding(
                doc_of(target, bytes, first, count, Some(Arc::new(strip)), runs),
                held,
            ))
        }
        // HEIC has no decoder here, and AVIF has one only in a build that kept it: a plugin may.
        Err(
            error @ (ImageError::Unsupported {
                format: RasterFormat::Heic,
            }
            | ImageError::NotCompiledIn {
                format: RasterFormat::Avif,
            }),
        ) => plugin_into(target, bytes, error),
        Err(error) => Err(error.into()),
    }
}
/// A picture only a plugin can decode: its pixels, or the facts and the row naming the package.
/// `unserved` is what the viewer says when no plugin is installed and none is known.
fn plugin_into(
    target: &RasterOpen,
    bytes: &[u8],
    unserved: ImageError,
) -> Result<RasterDoc, OpenError> {
    match target
        .plugins
        .decode(&target.source, &target.sniffed, PLUGIN_AREA)
    {
        PluginPicture::Pixels(picture) => {
            let held = upload(&target.texture, &picture)?;
            Ok(RasterDoc {
                offer: EditOffer::Copies(copy_target(&target.sniffed)),
                ..holding(
                    doc_of(target, bytes, picture.size(), 1, None, Runs::Forever),
                    held,
                )
            })
        }
        PluginPicture::Missing(needs) => Ok(blank_doc(target, bytes, needs)),
        PluginPicture::Unserved => Err(unserved.into()),
        PluginPicture::Failed(reason) => Err(OpenError::Plugin(reason)),
    }
}
/// A camera raw file: the plugin's full development when one is installed and works; else the
/// preview inside the file, with a row offering the plugin for the full picture; else, for a file
/// with no readable preview, the facts and that row.
fn raw_into(target: &RasterOpen, bytes: &[u8]) -> Result<RasterDoc, OpenError> {
    let developed = target
        .plugins
        .decode(&target.source, &target.sniffed, PLUGIN_AREA);
    let needs = match developed {
        PluginPicture::Pixels(picture) => {
            let held = upload(&target.texture, &picture)?;
            return Ok(RasterDoc {
                offer: EditOffer::Copies(copy_target(&target.sniffed)),
                ..holding(
                    doc_of(target, bytes, picture.size(), 1, None, Runs::Forever),
                    held,
                )
            });
        }
        PluginPicture::Missing(needs) => Some(needs),
        PluginPicture::Failed(reason) => {
            log::warn!("the raw plugin could not develop the file: {reason}");
            None
        }
        PluginPicture::Unserved => None,
    };
    match decode_bytes(bytes, &target.sniffed) {
        Ok(Decoded::Still(picture)) => {
            let held = upload(&target.texture, &picture)?;
            let mut doc = holding(
                doc_of(target, bytes, picture.size(), 1, None, Runs::Forever),
                held,
            );
            if let Some(needs) = needs {
                doc.facts = doc.facts.with(needs.fact.label, needs.fact.value);
                doc.lacking = needs.helper;
            }
            Ok(doc)
        }
        Ok(Decoded::Animated(_) | Decoded::HeldStill { .. }) => Err(OpenError::Unrecognised),
        Err(ImageError::NoPreview) => match needs {
            Some(needs) => Ok(blank_doc(target, bytes, needs)),
            None => Err(ImageError::NoPreview.into()),
        },
        Err(error) => Err(error.into()),
    }
}
/// A document with no picture: the facts of the file and the row naming what would show it.
fn blank_doc(target: &RasterOpen, bytes: &[u8], needs: Need) -> RasterDoc {
    let one = PixelSize {
        width: PixelLen(1),
        height: PixelLen(1),
    };
    let mut doc = doc_of(target, bytes, one, 1, None, Runs::Forever);
    doc.facts = Facts::empty()
        .with(
            FactLabel::Kind,
            crate::families::kind_row(&target.source, &target.sniffed),
        )
        .with(FactLabel::Size, FactValue::size(target.source.stamp().len))
        .with(needs.fact.label, needs.fact.value.clone());
    doc.lacking = needs.helper;
    doc.needs = Some(needs);
    doc.offer = EditOffer::Withheld;
    doc
}
/// `doc` for a texture that holds the picture at `held`, which is smaller than the picture when
/// the GPU cannot take it whole.
fn holding(doc: RasterDoc, held: PixelSize) -> RasterDoc {
    RasterDoc { held, ..doc }
}
