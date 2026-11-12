//! A gesture's sidecar writes read back into the index, where they ran, and the gallery laid from it.

use super::*;

impl Ctl {
    /// Rows whose sidecar a gesture wrote, read again into the index on this thread, then the
    /// gallery laid from the index, so a photo marked or restored leaves or joins it.
    pub(in crate::app) fn reread(self: &Rc<Self>, paths: &[String]) {
        let reread = self.state.borrow().reread_here(paths);
        self.reread_landed(paths, reread);
    }

    /// The gallery laid from rows a gesture's sidecar writes left in the index; rows cut for a
    /// folder no longer shown are left out.
    pub(in crate::app) fn reread_landed(self: &Rc<Self>, paths: &[String], reread: library::Reread) {
        if paths.is_empty() {
            return;
        }
        self.navigator_wrote(paths);
        {
            let mut s = self.state.borrow_mut();
            // A sidecar written may measure the roll shown or change its dyes: its card is read again.
            s.org.frames_due = true;
            // With no folder shown, a photo restored from the ONI Trash leaves it all the same.
            let shown = s.folder.as_mut().filter(|f| reread.folder.as_deref() == Some(f.path.as_str()));
            if let Some(shown) = shown {
                let mut rows = reread.rows;
                let held: HashSet<String> = rows.iter().map(|e| e.path.clone()).collect();
                let waiting: Vec<oni_library::Entry> =
                    shown.rows.iter().filter(|r| r.fingerprint.starts_with(thumbnails::PENDING) && !held.contains(&r.path)).cloned().collect();
                rows.extend(waiting);
                shown.copies = reread.copies;
                shown.rows = rows;
                let fingerprints = s.rows().iter().map(|e| (e.path.clone(), s.tile_key(e))).collect();
                s.fingerprints = fingerprints;
            }
        }
        self.lay_archive();
    }
}

impl State {
    /// `library::reread` on this thread's connection, for writes that ran on it.
    pub(in crate::app) fn reread_here(&self, paths: &[String]) -> library::Reread {
        match &self.index {
            Some(index) if !paths.is_empty() => library::reread(index, paths, self.shown()),
            _ => library::Reread::default(),
        }
    }
}
