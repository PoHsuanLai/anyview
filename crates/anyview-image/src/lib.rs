//! Images for the viewer: decode, a downscaled peek, EXIF facts and orientation, encode for
//! export, and lossless JPEG rotation. Blocking and pure of the runtime: every function runs on
//! the caller's worker, and none spawns or reads a clock.
//!
//! Every public item is reached from this root, once.
