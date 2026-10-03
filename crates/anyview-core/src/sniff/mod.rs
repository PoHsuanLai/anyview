//! Sniffing: what a file is, read from its bytes first and its name only as a fallback.
//!
//! `sniff` answers from the first 4 KiB. A zip needs a second step: the caller lists its entries
//! and `sniff_zip` names the zip-based format. Both return a [`Sniffed`], which nothing else can
//! make.

mod detect;
mod extension;
mod head;
mod magic;
mod sniffed;
mod text;
mod zip;

#[cfg(test)]
mod tests;

pub use detect::{sniff, sniff_folder};
pub use head::FileHead;
pub use sniffed::{SniffStep, Sniffed, ZipProbe};
pub use zip::{ZipEntries, sniff_zip};
