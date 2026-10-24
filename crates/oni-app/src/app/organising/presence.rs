//! What is there, as the interface holds it: each photo's and the opened folder's state, landed by
//! the reads and by looks off this thread. The interface thread reads these and never the disk.

use super::*;
use oni_library::paths::holds;
use oni_library::presence::{self, Away, Presence};

impl State {
    /// A photo's state as last published; a volume gone since makes it offline from its path alone.
    pub fn presence(&self, path: &str) -> Presence {
        if self.org.left.iter().any(|v| holds(v, path)) {
            return Presence::Offline(Away::Disconnected);
        }
        self.org.presence.get(path).copied().unwrap_or_default()
    }

    /// The opened folder's state, kept whatever it is: a folder away stays the one opened.
    pub fn opened_presence(&self) -> Presence {
        match self.opened.as_deref() {
            Some(o) if self.org.left.iter().any(|v| holds(v, o)) => Presence::Offline(Away::Disconnected),
            Some(_) => self.org.opened_presence,
            None => Presence::Present,
        }
    }

    /// The opened folder's reads started since launch.
    pub fn reads(&self) -> usize {
        self.org.reads
    }

    /// Each row's state and the opened folder's, cut from the survey they landed with.
    pub(in crate::app) fn landed_presence(&mut self) {
        let survey = self.survey.clone();
        for e in &self.rows {
            self.org.presence.insert(e.path.clone(), survey.presence(&e.path));
        }
        self.org.opened_presence = self.opened.as_deref().map(|o| survey.presence(o)).unwrap_or_default();
    }
}

impl Ctl {
    /// The originals of the photos held looked at again off this thread, as a read's end and a
    /// disk's return ask; the tiles follow once the looks land.
    pub fn probe_presence(self: &Rc<Self>) {
        let paths: Vec<String> = {
            let s = self.state.borrow();
            let mut paths: HashSet<String> = s.rows.iter().map(|e| e.path.clone()).collect();
            paths.extend(s.order.iter().cloned());
            paths.into_iter().collect()
        };
        self.probe(paths);
    }

    /// `paths` looked at on a thread of their own, each answer landing in `presence`.
    pub(in crate::app) fn probe(&self, paths: Vec<String>) {
        if paths.is_empty() {
            return;
        }
        let computer = self.state.borrow().nav.computer.clone();
        let _ = std::thread::Builder::new().name("presence".into()).spawn(move || {
            let found: Vec<(String, Presence)> = paths.into_iter().map(|p| {
                let state = presence::of(&p, &computer);
                (p, state)
            }).collect();
            super::super::with_shell(move |c| c.presence_landed(found));
        });
    }

    fn presence_landed(self: &Rc<Self>, found: Vec<(String, Presence)>) {
        let moved = {
            let mut s = self.state.borrow_mut();
            let mut moved = false;
            for (path, state) in found {
                moved |= s.org.presence.insert(path, state) != Some(state);
            }
            moved
        };
        if moved {
            self.lay_grid();
        }
    }
}
