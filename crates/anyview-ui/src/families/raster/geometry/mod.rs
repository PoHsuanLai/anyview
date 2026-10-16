//! Where a picture lands in the room it has: which texels show and where they are drawn. Pure
//! arithmetic over the stage machine's state, tested as tables. Content points and scales are the
//! machine's integers (`DocPoint` in 1/64 texel, `Permille`); a float appears inside the
//! arithmetic only.
//!
//! Everything the machine knows is in the picture *as shown*, that is, after its turn; the texture
//! is always the upright picture. So the layout works out the visible part in shown coordinates
//! and maps it back to the texture's own with the inverse of the turn, and the view draws the
//! texture's box rotated about its middle.

mod place;
mod pointer;
mod scale;

pub(crate) use place::{held_source, in_file, place};
pub(crate) use pointer::{limited_pan, point_under, pointer_delta, reach_in_picture, room_point};
pub(crate) use scale::{animation_of, centre_of, fit, frame_of, scale_of, turn_of};

#[cfg(test)]
mod tests;
