//! A player that cannot take the media thread down: a panic inside it is caught at the seam and
//! becomes the player failing, which the window says in words, instead of a thread that ends and a
//! line that goes quiet.

use anyview_media::{Continuation, Handled, MediaCommand, MediaDriver, MediaEvent};
use std::panic::{AssertUnwindSafe, catch_unwind};

/// What a player that panicked says.
const BROKEN: &str = "the player stopped unexpectedly";

/// A driver whose calls are caught, and which does nothing once one has panicked.
pub(super) struct Guarded<D> {
    inner: D,
    broken: Broken,
}

/// Whether the driver has panicked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Broken {
    No,
    Yes,
}

impl<D: MediaDriver> Guarded<D> {
    pub(super) fn new(inner: D) -> Guarded<D> {
        Guarded {
            inner,
            broken: Broken::No,
        }
    }

    fn broke(&mut self) -> MediaEvent {
        self.broken = Broken::Yes;
        eprintln!("anyview: the player panicked");
        MediaEvent::Failed(BROKEN.to_owned())
    }
}

impl<D: MediaDriver> MediaDriver for Guarded<D> {
    fn command(&mut self, command: MediaCommand) -> Handled {
        if command == MediaCommand::Close {
            return Handled {
                events: Vec::new(),
                then: Continuation::Close,
            };
        }
        if self.broken == Broken::Yes {
            return Handled {
                events: Vec::new(),
                then: Continuation::Keep,
            };
        }
        match catch_unwind(AssertUnwindSafe(|| self.inner.command(command))) {
            Ok(handled) => handled,
            Err(_) => Handled {
                events: vec![self.broke()],
                then: Continuation::Keep,
            },
        }
    }

    fn woken(&mut self) -> Vec<MediaEvent> {
        if self.broken == Broken::Yes {
            return Vec::new();
        }
        match catch_unwind(AssertUnwindSafe(|| self.inner.woken())) {
            Ok(events) => events,
            Err(_) => vec![self.broke()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A driver that panics when told to, so the seam is exercised.
    struct Panicky;

    impl MediaDriver for Panicky {
        fn command(&mut self, command: MediaCommand) -> Handled {
            assert!(!matches!(command, MediaCommand::Seek(_)), "a decoder bug");
            Handled {
                events: vec![MediaEvent::Volume(anyview_core::Volume::FULL)],
                then: Continuation::Keep,
            }
        }

        fn woken(&mut self) -> Vec<MediaEvent> {
            panic!("a decoder bug")
        }
    }

    #[test]
    fn a_panic_is_the_player_failing_and_it_stays_quiet_after() {
        let mut driver = Guarded::new(Panicky);
        let fine = driver.command(MediaCommand::SetVolume(anyview_core::Volume::FULL));
        assert_eq!(
            fine.events.len(),
            1,
            "a driver that works is passed through"
        );
        let hurt = driver.command(MediaCommand::Seek(anyview_core::MediaTime::default()));
        assert_eq!(hurt.events, vec![MediaEvent::Failed(BROKEN.to_owned())]);
        assert_eq!(hurt.then, Continuation::Keep);
        assert!(driver.woken().is_empty(), "nothing more is asked of it");
        let after = driver.command(MediaCommand::SetVolume(anyview_core::Volume::FULL));
        assert!(after.events.is_empty());
        assert_eq!(
            driver.command(MediaCommand::Close).then,
            Continuation::Close,
            "it can still be closed"
        );
    }

    #[test]
    fn a_panic_on_a_wake_is_caught_too() {
        let mut driver = Guarded::new(Panicky);
        assert_eq!(driver.woken(), vec![MediaEvent::Failed(BROKEN.to_owned())]);
    }
}
