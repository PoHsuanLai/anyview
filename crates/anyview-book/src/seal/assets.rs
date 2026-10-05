//! What a chapter refers to inside its package, read through a seam so sealing stays pure.

/// The files of the package a chapter may pull in.
pub(crate) trait Assets {
    /// The `data:` URL of the image or other file at the entry `entry`, or `None` when it is
    /// missing, too large, not an image or over what a chapter may inline.
    fn data_url(&self, entry: &str) -> Option<String>;
    /// The text of the stylesheet at the entry `entry`, or `None` when it cannot be read.
    fn text(&self, entry: &str) -> Option<String>;
}
