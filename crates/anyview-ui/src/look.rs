//! How a window looks, as data: the choices a `Ds` root resolves, and the feed that changes them
//! while the window is open. The window never reads a settings file itself; the program does, and
//! hands every window the same feed.

use ds::prelude::{Alpha, Appearance, SystemPrefs, Typeface};
use ds::style::material::stack::MaterialStack;

/// The desktop's look: theme, accent and motion, the system's scheme and contrast, and the
/// material and typeface keys. `None` leaves a key to the root's own default.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Look {
    /// Theme, accent, motion.
    pub appearance: Appearance,
    /// The system's scheme, contrast and reduced-motion answers.
    pub system: SystemPrefs,
    /// The tint of the window material, when the person set one.
    pub tint_alpha: Option<Alpha>,
    /// The typeface, when the person set one.
    pub typeface: Option<Typeface>,
    /// The material's highlight, hairline, shadow and vibrancy, when the person set them.
    pub stack: Option<MaterialStack>,
}

impl From<Appearance> for Look {
    /// The look of `appearance` on a desktop that says nothing else.
    fn from(appearance: Appearance) -> Look {
        Look {
            appearance,
            ..Look::default()
        }
    }
}

/// The look as it changes: a root context the binary provides to every window, so one watch of
/// the desktop's files serves all of them. A window with none keeps the look it was launched with.
#[derive(Debug, Clone)]
pub struct LookFeed(pub tokio::sync::watch::Receiver<Look>);

/// Two feeds are equal when they are the one channel, which is what a component's props need to know
/// whether the host handed over the same feed again.
impl PartialEq for LookFeed {
    fn eq(&self, other: &LookFeed) -> bool {
        self.0.same_channel(&other.0)
    }
}
