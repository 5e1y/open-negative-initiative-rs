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

/// The background work the editor supersedes, each kind under its own ticket.
#[derive(Default)]
pub struct Asks {
    /// The group of frames opened together last: an older group's decode or drawing lands nothing.
    pub groups: Ticket,
    /// The picture's histogram traces, and the levels' and curves' input traces: a trace overtaken by a later ask is dropped.
    pub traces: Ticket,
    pub input_traces: Ticket,
    /// The neighbour prefetch, which a later opening stops.
    pub near: Ticket,
    /// Report a Bug's sheet, opened last: a post answering a sheet closed since lands nowhere.
    pub report: Ticket,
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

    /// How many asks and supersessions so far: a number that moves with each, for a view to key on.
    pub fn count(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }

    /// The ask standing now, superseding none: several pieces of work under one ask share it.
    pub fn standing(&self) -> Asked {
        Asked { of: self.clone(), n: self.0.load(Ordering::Relaxed) }
    }
}

impl PartialEq for Asked {
    fn eq(&self, other: &Self) -> bool {
        self.n == other.n && Arc::ptr_eq(&self.of.0, &other.of.0)
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
        assert!(second == second.clone() && second != ticket.next(), "twin: an ask equals itself and no later one");
        assert_eq!(ticket.count(), 5, "each ask and each supersession moved the count");
    }
}
