//! Pictures only a plugin can decode, which an export of the picture is made from.

use anyview_core::{Sniffed, Source};
use anyview_image::Rgba8;

/// The plugins that decode the pictures the built-in decoders cannot (HEIC, AVIF in a build
/// without it, a camera raw file in full), as the program lends them to an export.
pub trait PluginPictures: Send + Sync + 'static {
    /// The upright picture of `source`, a file of `sniffed`'s type; none when no plugin serves it
    /// or the plugin could not make it. Blocking: it runs a program and waits for it.
    fn picture(&self, source: &Source, sniffed: &Sniffed) -> Option<Rgba8>;
}
