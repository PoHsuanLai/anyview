//! A picture as a `data:` URL, for markup that must not name a file.

use super::{FileHead, SniffStep, sniff};
use crate::kind::FormatKind;
use crate::source::FileName;

/// How much of a file's head decides whether it is a picture.
const HEAD: usize = 4096;

/// The MIME type of `bytes` when they sniff as a picture (raster or vector) under `name`, or
/// `None` for a file that sniffs as anything else.
pub fn image_mime(bytes: &[u8], name: &FileName) -> Option<String> {
    let head = FileHead::new(&bytes[..bytes.len().min(HEAD)]);
    match sniff(&head, name) {
        SniffStep::Done(sniffed)
            if matches!(sniffed.kind(), FormatKind::Raster | FormatKind::Vector) =>
        {
            Some(sniffed.mime().as_str().to_owned())
        }
        SniffStep::Done(_) | SniffStep::LookInside(_) => None,
    }
}

/// `bytes` as a base64 `data:` URL of their picture type, or `None` when they are not a picture.
pub fn image_data_url(bytes: &[u8], name: &FileName) -> Option<String> {
    let mime = image_mime(bytes, name)?;
    Some(format!(
        "data:{mime};base64,{}",
        ds_core::base64::encode(bytes)
    ))
}
