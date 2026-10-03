//! Naming a load, and a result tagged with the load that asked for it.

/// Names one load. Every result a worker sends back carries the ticket of the load that asked
/// for it, and a result whose ticket is not the current one is for a file the person has left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Ticket(pub u64);

impl Ticket {
    /// The ticket of the load after this one.
    pub fn next(self) -> Ticket {
        Ticket(self.0.saturating_add(1))
    }
}

/// A result and the ticket of the load that asked for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ticketed<T> {
    /// The load this result answers.
    pub ticket: Ticket,
    /// The result.
    pub value: T,
}

impl<T> Ticketed<T> {
    /// `value` for the load `ticket` names.
    pub fn new(ticket: Ticket, value: T) -> Self {
        Self { ticket, value }
    }

    /// The same ticket over another value.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Ticketed<U> {
        Ticketed {
            ticket: self.ticket,
            value: f(self.value),
        }
    }

    /// Whether this result answers the load `current` names; any other is for a file left behind.
    pub fn is_current(&self, current: Ticket) -> bool {
        self.ticket == current
    }
}
