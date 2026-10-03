//! Where the player's picture goes: the window's texture handle, which any thread may update.

use anyview_media::{FrameSink, TextureView};
use ds_blitz::TextureHandle;

/// Shows the player's texture through a window's [`TextureHandle`] and asks its windows to
/// repaint when a frame is drawn. The media thread calls this, so it only hands the news on.
pub(super) struct WindowSink(pub(super) TextureHandle);

impl FrameSink for WindowSink {
    fn texture(&mut self, view: &TextureView) {
        self.0.replace_view(view.clone());
    }

    fn frame(&mut self) {
        self.0.redraw();
    }

    fn cleared(&mut self) {
        self.0.clear();
    }
}

/// The picture of a session with no window: there is none to show.
pub(super) struct NoSink;

impl FrameSink for NoSink {
    fn texture(&mut self, _view: &TextureView) {}

    fn frame(&mut self) {}

    fn cleared(&mut self) {}
}
