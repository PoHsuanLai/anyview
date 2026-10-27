//! How a window looks, as data: the choices a `Ds` root resolves, and the feed that changes them
//! while the window is open. The window never reads a settings file itself; the program does, and
//! hands every window the same feed.

use ds::prelude::{Appearance, SystemPrefs, Typeface};
use ds::style::kit::{CustomStyles, Styles};
use ds_blitz::StylesGuard;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::task::{Context, Poll, Waker};

/// The desktop's look: theme, accent and motion, the system's scheme and contrast, and the
/// typeface key (the material's values are stylesheet tokens, set by a look's sheet). `None` leaves a key to the root's own default.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Look {
    /// Theme, accent, motion.
    pub appearance: Appearance,
    /// The system's scheme, contrast and reduced-motion answers.
    pub system: SystemPrefs,
    /// The typeface, when the person set one.
    pub typeface: Option<Typeface>,
    /// The chosen look's sheet and the person's two (`~/.config/quire/style.css`,
    /// `~/.config/anyview/style.css`), as the host read them. Blank by default.
    pub styles: Styles,
    /// Whether `styles` are drawn: the host's answer from `appearance.custom_styles` and the crash
    /// guard.
    pub custom_styles: CustomStyles,
    /// The crash guard of `styles`, when the host keeps one.
    pub rescue: Option<StylesRescue>,
}

/// The crash guard of the styles a window draws, and the way back from it: the window marks the
/// start good once it has lived long enough, and shows the "Turn On Again" line when the guard
/// started the program without the person's styles. The host makes it, since the guard is the
/// process's, not a window's.
#[derive(Clone)]
pub struct StylesRescue {
    guard: StylesGuard,
    turn_on: Arc<dyn Fn() + Send + Sync>,
}

impl StylesRescue {
    /// A rescue of `guard`, whose button runs `turn_on`: the host clears the guard and sends the
    /// look again with the styles drawn.
    #[must_use]
    pub fn new(guard: StylesGuard, turn_on: Arc<dyn Fn() + Send + Sync>) -> StylesRescue {
        StylesRescue { guard, turn_on }
    }

    /// The guard.
    #[must_use]
    pub fn guard(&self) -> &StylesGuard {
        &self.guard
    }

    /// The person pressed Turn On Again.
    pub fn turn_on_again(&self) {
        (self.turn_on)();
    }
}

impl fmt::Debug for StylesRescue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StylesRescue")
            .field("guard", &self.guard)
            .finish_non_exhaustive()
    }
}

impl PartialEq for StylesRescue {
    fn eq(&self, other: &StylesRescue) -> bool {
        self.guard == other.guard && Arc::ptr_eq(&self.turn_on, &other.turn_on)
    }
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

/// The look as it changes: a root context the binary (or an embedding host) provides to every
/// window, so one watch of the desktop's files serves all of them. A window with none keeps the
/// look it was launched with.
///
/// A feed is the receiving end of [`LookFeed::new`]. It clones freely, and every clone sees the
/// same changes. It names no async runtime: [`changed`](LookFeed::changed) is an ordinary future.
#[derive(Clone)]
pub struct LookFeed {
    shared: Arc<Shared>,
    seen: u64,
}

/// The sending end of a [`LookFeed`]: the one place the look is changed. Dropping it ends the
/// feed, and every [`LookFeed::changed`] then answers `None` once it has caught up.
pub struct LookSender {
    shared: Arc<Shared>,
}

struct Shared {
    state: Mutex<State>,
}

struct State {
    look: Look,
    version: u64,
    open: bool,
    waiting: Vec<Waker>,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl LookFeed {
    /// A feed that starts at `initial`, and the sender that changes it.
    pub fn new(initial: Look) -> (LookSender, LookFeed) {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                look: initial,
                version: 0,
                open: true,
                waiting: Vec::new(),
            }),
        });
        let sender = LookSender {
            shared: Arc::clone(&shared),
        };
        (sender, LookFeed { shared, seen: 0 })
    }

    /// The look now.
    pub fn current(&self) -> Look {
        self.shared.lock().look.clone()
    }

    /// Wait for a look this feed has not yet returned, and return it. `None` when the sender is
    /// gone and there is nothing newer.
    pub async fn changed(&mut self) -> Option<Look> {
        std::future::poll_fn(|cx| self.poll_changed(cx)).await
    }

    fn poll_changed(&mut self, cx: &mut Context<'_>) -> Poll<Option<Look>> {
        let mut state = self.shared.lock();
        if state.version != self.seen {
            self.seen = state.version;
            return Poll::Ready(Some(state.look.clone()));
        }
        if !state.open {
            return Poll::Ready(None);
        }
        if !state.waiting.iter().any(|w| w.will_wake(cx.waker())) {
            state.waiting.push(cx.waker().clone());
        }
        Poll::Pending
    }
}

impl LookSender {
    /// Make `look` the feed's look and wake every window following it. A look equal to the current
    /// one changes nothing and wakes no one.
    pub fn set(&self, look: Look) {
        let waiting = {
            let mut state = self.shared.lock();
            if state.look == look {
                return;
            }
            state.look = look;
            state.version += 1;
            std::mem::take(&mut state.waiting)
        };
        waiting.into_iter().for_each(Waker::wake);
    }
}

impl Drop for LookSender {
    fn drop(&mut self) {
        let waiting = {
            let mut state = self.shared.lock();
            state.open = false;
            std::mem::take(&mut state.waiting)
        };
        waiting.into_iter().for_each(Waker::wake);
    }
}

impl fmt::Debug for LookFeed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LookFeed")
            .field("look", &self.current())
            .finish()
    }
}

impl fmt::Debug for LookSender {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LookSender").finish_non_exhaustive()
    }
}

/// Two feeds are equal when they are the one channel, which is what a component's props need to know
/// whether the host handed over the same feed again.
impl PartialEq for LookFeed {
    fn eq(&self, other: &LookFeed) -> bool {
        Arc::ptr_eq(&self.shared, &other.shared)
    }
}
