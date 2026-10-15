//! Where the probe of the load in flight stands, and the family of stage a probed file gets.

use crate::Ticket;
use crate::families::family_of;
use crate::io::Opened;

/// Where the probe of the load in flight stands. A probe's result is announced to the load machine
/// only after the window has drawn once with it, so the parameters the machine steps with already
/// know the file's kind (which views a text file has).
#[derive(Debug, Clone, PartialEq)]
pub(in crate::views) enum Probe {
    /// No file has been asked for.
    Idle,
    /// The probe of this ticket is running.
    Pending(Ticket),
    /// The probe of this ticket is running again for a file that changed; the second is what the
    /// file on screen was probed as.
    Reprobing(Ticket, Opened),
    /// The probe answered; the load machine has not been told yet.
    Arrived(Ticket, Opened),
    /// The load machine knows.
    Announced(Ticket, Opened),
}

impl Probe {
    /// What the probe found, once it has.
    pub(in crate::views) fn found(&self) -> Option<&Opened> {
        match self {
            Probe::Arrived(_, probed)
            | Probe::Announced(_, probed)
            | Probe::Reprobing(_, probed) => Some(probed),
            Probe::Idle | Probe::Pending(_) => None,
        }
    }

    /// The load this probe belongs to.
    pub(in crate::views) fn ticket(&self) -> Option<Ticket> {
        match self {
            Probe::Pending(ticket)
            | Probe::Reprobing(ticket, _)
            | Probe::Arrived(ticket, _)
            | Probe::Announced(ticket, _) => Some(*ticket),
            Probe::Idle => None,
        }
    }
}

/// The family of stage a probed file gets.
pub(in crate::views) fn family(probed: &Opened) -> crate::StageFamily {
    family_of(probed.sniffed.kind())
}
