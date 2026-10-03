//! Where the player's picture goes.

/// The window's end of the picture. The player draws into a texture of its own; the sink tells
/// whoever shows it. Called on the media thread, so an implementation only hands the news on
/// (to a texture handle that any thread may update, and a redraw request).
pub trait FrameSink: Send + 'static {
    /// The player's picture is now this texture: it was made, or replaced because the slot
    /// changed size. Show it from now on.
    fn texture(&mut self, view: &wgpu::TextureView);
    /// The player drew a new frame into that texture.
    fn frame(&mut self);
    /// The player has no picture any more (an audio file replaced a video).
    fn cleared(&mut self);
}
