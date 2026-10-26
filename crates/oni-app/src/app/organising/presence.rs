//! What is there, as the interface holds it: each photo's and the opened folder's state, landed by
//! the reads and by looks off this thread. The interface thread reads these and never the disk.

use super::*;
use oni_library::paths::holds;
use std::path::Path;
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
            let state = survey.presence(&e.path);
            // Each path's key kept, never cloned again, as every landing passes every row.
            match self.org.presence.get_mut(&e.path) {
                Some(held) => *held = state,
                None => {
                    self.org.presence.insert(e.path.clone(), state);
                }
            }
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
        let (computer, leads) = {
            let s = self.state.borrow();
            let leads: HashSet<String> = s.rows.iter().filter(|e| e.leads_passes).map(|e| e.path.clone()).collect();
            (s.nav.computer.clone(), leads)
        };
        let _ = std::thread::Builder::new().name("presence".into()).spawn(move || {
            let found: Vec<(String, Presence)> = paths.iter().map(|p| (p.clone(), presence::of(p, &computer))).collect();
            // A lead there whose triplet lost a pass: the first one gone, by its channel.
            let passes: Vec<(String, Option<(usize, String)>)> = found
                .iter()
                .filter(|(p, state)| state.is_present() && leads.contains(p))
                .map(|(lead, _)| (lead.clone(), gone_pass(lead, &computer)))
                .collect();
            // A file no reader of ONI takes, by its format's name: only one its name leaves in doubt is opened.
            let unsupported: Vec<(String, Option<String>)> = found
                .iter()
                .filter(|(p, state)| state.is_present() && !oni_decode::named_decodable(Path::new(p)))
                .map(|(p, _)| (p.clone(), oni_decode::unsupported_format(Path::new(p))))
                .collect();
            super::super::with_shell(move |c| c.presence_landed(found, passes, unsupported));
        });
    }

    fn presence_landed(self: &Rc<Self>, found: Vec<(String, Presence)>, passes: Vec<(String, Option<(usize, String)>)>, unsupported: Vec<(String, Option<String>)>) {
        let moved = {
            let mut s = self.state.borrow_mut();
            let mut moved = false;
            for (path, state) in found {
                moved |= s.org.presence.insert(path, state) != Some(state);
            }
            for (lead, gone) in passes {
                let was = match gone {
                    Some(gone) => s.org.seen.gone_passes.insert(lead, gone.clone()) != Some(gone),
                    None => s.org.seen.gone_passes.remove(&lead).is_some(),
                };
                moved |= was;
            }
            for (path, format) in unsupported {
                moved |= match format {
                    Some(format) => s.org.seen.unsupported.insert(path, format.clone()) != Some(format),
                    None => s.org.seen.unsupported.remove(&path).is_some(),
                };
            }
            moved
        };
        if moved {
            self.lay_grid();
        }
    }
}

/// The first of `lead`'s passes its sidecar names that is not beside it. Reads the disk.
fn gone_pass(lead: &str, computer: &oni_library::navigator::Computer) -> Option<(usize, String)> {
    let passes = oni_library::rgb_passes::named_by(lead)?;
    let names = [&passes.red, &passes.green, &passes.blue];
    oni_library::rgb_passes::paths_beside(&passes, lead)
        .iter()
        .position(|p| !presence::of(p, computer).is_present())
        .map(|channel| (channel, names[channel].clone()))
}
