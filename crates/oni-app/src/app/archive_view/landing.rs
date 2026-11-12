//! A folder's read off this thread and its landings: the rows, the tree cut beside them, the copies
//! and the tile keys, taken on this thread as they come.

use super::*;
use crate::app::folder::Folder;
use crate::ticket::Asked;

/// What a read cuts beside the rows it lands, off the interface thread.
pub(super) struct Landed {
    /// The archive of these rows under the filter at the read's start.
    staged: Archive,
    /// The prints two rows of the index hold.
    copies: HashSet<String>,
    /// Each row's tile key under those copies.
    fingerprints: HashMap<String, String>,
}

impl Ctl {
    /// The read on its own thread and its own connection; each landing comes back to this thread.
    /// A read asked after it stops it and lands nothing, so an older survey never lands after a newer one.
    pub(super) fn read_folder(self: &Rc<Self>, folder: String, index_path: PathBuf) {
        let (asked, computer, filter) = {
            let mut s = self.state.borrow_mut();
            if s.read_waits() {
                return;
            }
            s.org.reads += 1;
            (s.reads.next(), s.nav.computer.clone(), s.filter)
        };
        std::thread::Builder::new()
            .name("library".into())
            .spawn(move || {
                let Some(index) = LibraryIndex::open(&index_path) else {
                    return with_shell(|c| c.told(tr("Something Went Wrong"), tr("The library index cannot be opened")));
                };
                let alive = || asked.current();
                let kept = index_path.parent().map(|caches| caches.join("Surveys"));
                let place = library::Place { computer: &computer, kept: kept.as_deref() };
                library::read_rows(&folder, &index, &place, &alive, &mut |rows, done, survey| {
                    // The tree, the copies and the tile keys are cut here, so a landing costs the interface no cut.
                    let copies = index.copies();
                    let fingerprints = rows.iter().map(|e| (e.path.clone(), thumbnails::key(&e.fingerprint, &e.path, &copies))).collect();
                    let landed = Landed { staged: Archive::from_rows(&folder, &rows, survey, filter), copies, fingerprints };
                    let survey = survey.clone();
                    let (asked, folder) = (asked.clone(), folder.clone());
                    with_shell(move |c| c.take_rows(asked, folder, rows, done, survey, landed));
                });
            })
            .expect("library thread");
    }

    /// The read under way stops and lands nothing, the folder left to the next read.
    pub(in crate::app) fn read_superseded(&self) {
        self.state.borrow().reads.supersede();
    }

    /// `takeRows` then `layGallery`: the folder read replaces the one shown whole, its rows laid,
    /// the selection and the open photo kept by path.
    pub(super) fn take_rows(self: &Rc<Self>, asked: Asked, path: String, rows: Vec<oni_library::Entry>, done: bool, survey: Arc<Survey>, landed: Landed) {
        {
            let mut s = self.state.borrow_mut();
            if !asked.current() {
                return;
            }
            s.folder = Some(Folder { path, rows, survey, copies: landed.copies, staged: Some(landed.staged), read_done: done });
            s.heading = None;
            s.fingerprints = landed.fingerprints;
            s.org.reads_whole += usize::from(done);
            s.landed_presence();
        }
        self.lay_archive();
        if done {
            self.tidy_once();
            self.probe_presence();
            self.navigator_read();
            let order = self.state.borrow().order.clone();
            self.offer_neutral_decode_notice(&order);
            self.dng_folder_read();
            self.fill();
            let opening = self.state.borrow_mut().opening.take();
            let found = opening.and_then(|o| {
                let s = self.state.borrow();
                if o.is_empty() { s.order.first().cloned() } else { s.order.iter().find(|p| **p == o).cloned() }
            });
            if let Some(path) = found {
                self.open_in_editor(&path);
            }
            self.wrapped_after_read();
            self.read_once_settled();
        }
    }

    /// A read that left files still being written reads the folder again once they may rest, so a
    /// copy that ends with no further change is still printed whole.
    fn read_once_settled(self: &Rc<Self>) {
        let (settling, opening) = {
            let s = self.state.borrow();
            (!s.survey().settling.is_empty(), s.folder_turns.standing())
        };
        if !settling {
            return;
        }
        let me = Rc::downgrade(self);
        Timer::single_shot(oni_library::roots::SETTLE * 2, move || {
            if let Some(me) = me.upgrade()
                && opening.current()
            {
                me.read_again();
            }
        });
    }
}
