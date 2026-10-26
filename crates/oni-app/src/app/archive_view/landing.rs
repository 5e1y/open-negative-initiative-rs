//! A folder's read off this thread and its landings: the rows, the tree cut beside them, the copies
//! and the tile keys, taken on this thread as they come.

use super::*;

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
    pub(super) fn read_folder(self: &Rc<Self>, generation: u64, folder: String, index_path: PathBuf) {
        let (current, computer, filter) = {
            let mut s = self.state.borrow_mut();
            s.org.reads += 1;
            (s.generation.clone(), s.nav.computer.clone(), s.filter)
        };
        std::thread::Builder::new()
            .name("library".into())
            .spawn(move || {
                let Some(index) = LibraryIndex::open(&index_path) else {
                    return with_shell(|c| c.told(tr("Something Went Wrong"), tr("The library index cannot be opened")));
                };
                let alive = || current.load(Ordering::Relaxed) == generation;
                let kept = index_path.parent().map(|caches| caches.join("Surveys"));
                let place = library::Place { computer: &computer, kept: kept.as_deref() };
                library::read_rows(&folder, &index, &place, &alive, &mut |rows, done, survey| {
                    // The tree, the copies and the tile keys are cut here, so a landing costs the interface no cut.
                    let copies = index.copies();
                    let fingerprints = rows.iter().map(|e| (e.path.clone(), thumbnails::key(&e.fingerprint, &e.path, &copies))).collect();
                    let landed = Landed { staged: Archive::from_rows(&folder, &rows, survey, filter), copies, fingerprints };
                    let survey = survey.clone();
                    with_shell(move |c| c.take_rows(generation, rows, done, survey, landed));
                });
            })
            .expect("library thread");
    }

    /// `takeRows` then `layGallery`: the rows laid, the selection and the open photo kept by path.
    pub(super) fn take_rows(self: &Rc<Self>, generation: u64, rows: Vec<oni_library::Entry>, done: bool, survey: Arc<Survey>, landed: Landed) {
        {
            let mut s = self.state.borrow_mut();
            if s.generation.load(Ordering::Relaxed) != generation {
                return;
            }
            s.copies = landed.copies;
            s.org.staged = Some(landed.staged);
            s.fingerprints = landed.fingerprints;
            s.rows = rows;
            s.survey = survey;
            s.read_done = done;
            s.landed_presence();
        }
        self.lay_archive();
        if done {
            self.tidy_once();
            self.probe_presence();
            self.navigator_read();
            let order = self.state.borrow().order.clone();
            self.offer_neutral_decode_notice(&order);
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
        let (settling, generation) = {
            let s = self.state.borrow();
            (!s.survey.settling.is_empty(), s.generation.load(Ordering::Relaxed))
        };
        if !settling {
            return;
        }
        let me = Rc::downgrade(self);
        Timer::single_shot(oni_library::roots::SETTLE * 2, move || {
            if let Some(me) = me.upgrade()
                && me.state.borrow().generation.load(Ordering::Relaxed) == generation
            {
                me.read_again();
            }
        });
    }
}
