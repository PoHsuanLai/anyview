//! What a worker made, as input to the machines. Every result carries the ticket of the file it
//! was made for; the machines drop one for a file the person has left.

mod find;
mod media;
mod open;
mod results;

pub(super) use media::drain_media;
pub(super) use open::landed;
pub(super) use results::{arrived, shown_path};
