//! The folder the archive shows: its path and what its read landed, owned together so the next
//! folder's first landing replaces all of it at once, the old tiles shown until then.

use super::*;
use std::sync::LazyLock;

pub(in crate::app) struct Folder {
    pub path: String,
    /// Every row the index shows under it, before the filter.
    pub rows: Vec<oni_library::Entry>,
    pub survey: Arc<Survey>,
    /// The prints two rows of the index hold.
    pub copies: HashSet<String>,
    /// The archive cut beside the rows off this thread, which the next lay takes.
    pub staged: Option<Archive>,
    /// Its last read landed whole.
    pub read_done: bool,
}

/// The survey of no folder, before any landed.
static NONE: LazyLock<Arc<Survey>> = LazyLock::new(Arc::default);

impl State {
    /// The folder asked last, landed or not: the navigator's row and the folder remembered.
    pub fn opened(&self) -> Option<&str> {
        self.heading.as_deref().or(self.shown())
    }

    /// The folder whose rows the archive shows.
    pub fn shown(&self) -> Option<&str> {
        self.folder.as_ref().map(|f| f.path.as_str())
    }

    /// The folder asked last read whole.
    pub fn read_done(&self) -> bool {
        self.heading.is_none() && self.folder.as_ref().is_some_and(|f| f.read_done)
    }

    /// The shown folder's rows, none before a first landing.
    pub(in crate::app) fn rows(&self) -> &[oni_library::Entry] {
        self.folder.as_ref().map_or(&[], |f| &f.rows)
    }

    pub(in crate::app) fn survey(&self) -> &Arc<Survey> {
        self.folder.as_ref().map_or(&NONE, |f| &f.survey)
    }

    /// The name a row's tile goes by (`thumbnails::key`): a copy's is told from the other's.
    pub(super) fn tile_key(&self, e: &oni_library::Entry) -> String {
        match &self.folder {
            Some(f) => thumbnails::key(&e.fingerprint, &e.path, &f.copies),
            None => thumbnails::key(&e.fingerprint, &e.path, &HashSet::new()),
        }
    }
}
