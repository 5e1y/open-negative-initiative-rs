//! Work asked off the interface thread under a ticket: a newer ask supersedes the older, whose
//! answer lands nothing.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// The counter every ask of one kind is numbered from; a clone shares it.
#[derive(Clone, Default)]
pub struct Ticket(Arc<AtomicU64>);

/// One ask, current until the ticket is taken again or superseded.
#[derive(Clone)]
pub struct Asked {
    of: Ticket,
    n: u64,
}

impl Ticket {
    /// A new ask, which supersedes every older one.
    pub fn next(&self) -> Asked {
        let n = self.0.fetch_add(1, Ordering::Relaxed) + 1;
        Asked { of: self.clone(), n }
    }

    /// Every ask so far superseded, with none newer.
    pub fn supersede(&self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }

    /// The ask standing now, superseding none: several pieces of work under one ask share it.
    pub fn standing(&self) -> Asked {
        Asked { of: self.clone(), n: self.0.load(Ordering::Relaxed) }
    }
}

impl Asked {
    pub fn current(&self) -> bool {
        self.of.0.load(Ordering::Relaxed) == self.n
    }
}

#[cfg(test)]
mod tests {
    use super::Ticket;

    #[test]
    fn an_ask_stays_current_until_a_newer_one_or_a_supersession() {
        let ticket = Ticket::default();
        let first = ticket.next();
        assert!(first.current());
        let standing = ticket.standing();
        assert!(standing.current(), "the standing ask shares the last one's number");
        let second = ticket.next();
        assert!(!first.current() && !standing.current(), "twin: a newer ask leaves the older ones behind");
        assert!(second.current());
        ticket.supersede();
        assert!(!second.current(), "twin: a supersession leaves the last ask behind");
        assert!(ticket.clone().next().current() && !second.current(), "a clone counts on the same ticket");
    }
}
