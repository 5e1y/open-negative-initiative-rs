//! What a project reaches: the opened folder's rows and every member the index holds beyond them,
//! so the Projects tab and every project gesture read the whole library, whatever folder is open.

use super::*;
use std::borrow::Cow;

impl State {
    /// The rows projects resolve against: the opened folder's first, then the members beyond it.
    pub(in crate::app) fn reach(&self) -> Cow<'_, [Entry]> {
        if self.org.standing.is_empty() {
            return Cow::Borrowed(&self.rows);
        }
        Cow::Owned(self.rows.iter().chain(&self.org.standing).cloned().collect())
    }
}

impl Ctl {
    /// `checkProjectPhotos`: the members beyond the opened folder read again off the index, one
    /// indexed look per member; the originals' states are looked at behind, as any tile's.
    pub(in crate::app) fn read_standing(&self) {
        let mut s = self.state.borrow_mut();
        let standing = match &s.index {
            Some(index) if !s.org.projects.is_empty() => oni_library::project_standing::standing(&s.org.projects, &s.rows, index),
            _ => Vec::new(),
        };
        s.org.standing = standing;
    }

    /// The ONI Trash and the projects' members, read for the whole library: a launch with no
    /// folder open lists them as one that opens a folder does.
    pub(in crate::app) fn read_library(&self) {
        self.read_trashed();
        self.read_standing();
    }
}
