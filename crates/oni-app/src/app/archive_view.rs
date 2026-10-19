//! The archive: opening a folder, reading its rows, the tree, the thumbnails' pass and opening a photo.

use super::*;

impl Ctl {
    // MARK: - The archive

    /// `add(_:)`: a folder opens; a file opens its folder, then itself in the editor once found.
    pub(super) fn add(self: &Rc<Self>, path: &Path) {
        let file = !path.is_dir();
        let folder = if file { path.parent().unwrap_or(path) } else { path };
        let opening = (file && oni_library::library::is_importable(path)).then(|| oni_library::paths::canonical(path));
        self.navigator_reveal(folder);
        self.open_folder(folder);
        self.state.borrow_mut().opening = opening;
    }

    /// The launch's folder: `openedFolder` if it still exists, else the first root of `roots.json`.
    pub(super) fn open_remembered(self: &Rc<Self>) -> bool {
        let (remembered, support) = {
            let s = self.state.borrow();
            (s.preferences.string(preferences::OPENED_FOLDER).map(PathBuf::from), s.support.clone())
        };
        let roots = || oni_library::roots::Roots::load(&support).current().map(PathBuf::from);
        let folder = remembered.filter(|p| p.is_dir()).or_else(roots);
        match folder {
            Some(folder) => {
                self.open_folder(&folder);
                true
            }
            None => false,
        }
    }

    /// `open()`: the system's folder picker, then `add`; a cancel leaves everything as it was.
    pub(super) fn pick(self: &Rc<Self>) {
        // Taken out first: a modal panel runs a nested event loop, which may reach the state.
        let picker = std::mem::replace(&mut self.state.borrow_mut().picker, Box::new(|| None));
        let chosen = picker();
        self.state.borrow_mut().picker = picker;
        if let Some(folder) = chosen {
            self.add(&folder);
        } else if !system::HAS_PICKER {
            self.say("This system has no folder picker: pass the folder on the command line");
        }
    }

    /// `openFolder`: the gallery on the folder, read through the index behind, its thumbnails
    /// drawn once the read is whole; the editor keeps what it holds.
    pub(super) fn open_folder(self: &Rc<Self>, folder: &Path) {
        let folder = oni_library::paths::canonical(folder);
        let (generation, index_path) = {
            let mut s = self.state.borrow_mut();
            s.org.trash_open = false;
            s.trash_picks = Default::default();
            s.opened = Some(folder.clone());
            s.reveal.fresh = true;
            let _ = s.preferences.set(preferences::OPENED_FOLDER, serde_json::Value::String(folder.clone()));
            s.opening = None;
            s.read_done = false;
            s.creating = (0, 0);
            s.painted = (0, 0);
            s.painter.0.lock().expect("painter").pass = None;
            (s.generation.fetch_add(1, Ordering::Relaxed) + 1, s.index_path.clone())
        };
        self.navigator_opened(&folder);
        self.read_folder(generation, folder, index_path);
    }

    /// `rescan`: the opened folder read again, the index trimmed to what the disk holds.
    pub(super) fn read_again(self: &Rc<Self>) {
        let (folder, index_path, generation) = {
            let mut s = self.state.borrow_mut();
            let Some(folder) = s.opened.clone() else { return };
            s.read_done = false;
            (folder, s.index_path.clone(), s.generation.load(Ordering::Relaxed))
        };
        self.read_folder(generation, folder, index_path);
    }

    /// The read on its own thread and its own connection; each landing comes back to this thread.
    pub(super) fn read_folder(self: &Rc<Self>, generation: u64, folder: String, index_path: PathBuf) {
        let current = self.state.borrow().generation.clone();
        std::thread::Builder::new()
            .name("library".into())
            .spawn(move || {
                let Some(index) = LibraryIndex::open(&index_path) else {
                    return with_shell(|c| c.say("The library index cannot be opened"));
                };
                let alive = || current.load(Ordering::Relaxed) == generation;
                library::read_rows(&folder, &index, &alive, &mut |rows, done, survey| {
                    let survey = survey.clone();
                    with_shell(move |c| c.take_rows(generation, rows, done, survey));
                });
            })
            .expect("library thread");
    }

    /// `takeRows` then `layGallery`: the rows laid, the selection and the open photo kept by path.
    pub(super) fn take_rows(self: &Rc<Self>, generation: u64, rows: Vec<oni_library::Entry>, done: bool, survey: Survey) {
        {
            let mut s = self.state.borrow_mut();
            if s.generation.load(Ordering::Relaxed) != generation {
                return;
            }
            s.fingerprints = rows.iter().map(|e| (e.path.clone(), e.fingerprint.clone())).collect();
            s.rows = rows;
            s.survey = survey;
            s.read_done = done;
        }
        self.lay_archive();
        if done {
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
        }
    }

    /// The archive cut again from the rows held, under the filter; what the hand folded stays so,
    /// except in a folder just opened.
    pub(super) fn lay_archive(self: &Rc<Self>) {
        let (title, stock) = {
            let mut s = self.state.borrow_mut();
            let Some(folder) = s.opened.clone() else { return };
            let mut archive = Archive::from_rows(&folder, &s.rows, &s.survey, s.filter);
            let fresh = std::mem::take(&mut s.reveal.fresh);
            if let Some(old) = s.archive.as_ref().filter(|_| !fresh) {
                let folded: HashSet<_> = old.nodes.iter().map(oni_library::archive_tree::Node::id).filter(|id| !old.expanded.contains(id)).collect();
                archive.expanded.retain(|id| !folded.contains(id));
            }
            let every: Vec<String> = s.rows.iter().map(|e| e.path.clone()).collect();
            s.store.set_archive(every);
            let first = archive.groups.first().map(|g| (g.title.clone(), g.stock.clone())).unwrap_or_default();
            s.archive = Some(archive);
            first
        };
        if let Some(w) = self.window() {
            let lib = w.global::<Library>();
            lib.set_roll_title(title.into());
            lib.set_stock(stock.into());
        }
        self.read_trashed();
        self.lay_grid();
        self.publish_tree();
        self.publish_bar();
    }

    /// `fillOpened`: the pass over the opened folder's rows, which draws those with no file.
    pub(super) fn fill(&self) {
        let s = self.state.borrow();
        let generation = s.generation.load(Ordering::Relaxed);
        let rows: Vec<(String, String)> = s
            .rows
            .iter()
            .filter(|e| !e.fingerprint.starts_with(thumbnails::PENDING))
            .map(|e| (e.path.clone(), e.fingerprint.clone()))
            .collect();
        let (lock, wake) = &*s.painter;
        lock.lock().expect("painter").pass = Some((generation, rows));
        wake.notify_one();
    }

    /// `invalidate`: the photo's redraw, queued once and drawn before the pass's next photo.
    pub(super) fn redraw(&self, path: &str) {
        let mut s = self.state.borrow_mut();
        let fingerprint = s.fingerprints.get(path).filter(|f| !f.starts_with(thumbnails::PENDING)).cloned();
        let painter = s.painter.clone();
        let (lock, wake) = &*painter;
        let mut paint = lock.lock().expect("painter");
        if !paint.redraws.iter().any(|(p, _)| p == path) {
            paint.redraws.push_back((path.to_string(), fingerprint));
            s.drawing += 1;
        }
        wake.notify_one();
    }

    /// The open photo's thumbnail `REFRESH_DELAY` after its sidecar is written and no hand holds.
    pub(super) fn schedule_redraw(self: &Rc<Self>) {
        let me = Rc::downgrade(self);
        self.refresher.start(TimerMode::SingleShot, thumbnails::REFRESH_DELAY, move || {
            let Some(me) = me.upgrade() else { return };
            let path = {
                let s = me.state.borrow();
                let held = s.hands.load(Ordering::Relaxed) > 0 || s.last_change.is_some();
                s.open.clone().filter(|_| !held)
            };
            if let Some(path) = path {
                me.redraw(&path);
            }
        });
    }

    /// The pass's count once it knows which photos lack a file.
    pub(super) fn pass_counted(&self, generation: u64, held: usize, absent: usize) {
        let mut s = self.state.borrow_mut();
        if s.generation.load(Ordering::Relaxed) == generation {
            s.painted = (held, held + absent);
            s.creating = (0, absent);
        }
    }

    /// A drawn thumbnail lands in memory and on every tile holding its photo; a failure says why.
    pub(super) fn thumb_landed(&self, pass: Option<u64>, fingerprint: String, drawn: Result<(Vec<u8>, usize, usize), String>, name: String) {
        let failure = {
            let mut s = self.state.borrow_mut();
            match pass {
                Some(g) if g == s.generation.load(Ordering::Relaxed) => {
                    s.painted.0 += 1;
                    s.creating.0 += 1;
                }
                Some(_) => {}
                None => s.drawing = s.drawing.saturating_sub(1),
            }
            match drawn {
                Ok((rgba, w, h)) => {
                    s.failed.remove(&fingerprint);
                    s.asked.remove(&fingerprint);
                    s.thumbs.insert(fingerprint, image(&rgba, w, h));
                    None
                }
                Err(e) => {
                    s.failed.insert(fingerprint);
                    Some(format!("{name}: {e}"))
                }
            }
        };
        if let Some(message) = failure {
            self.say(&message);
        }
        self.publish_tiles();
    }

    /// A file read from the folder; one absent stays asked, so only a render fills its tile.
    pub(super) fn disk_landed(&self, fingerprint: String, read: Option<(Vec<u8>, usize, usize)>) {
        let Some((rgba, w, h)) = read else { return };
        {
            let mut s = self.state.borrow_mut();
            s.asked.remove(&fingerprint);
            s.thumbs.insert(fingerprint, image(&rgba, w, h));
        }
        self.publish_tiles();
    }

    /// `offerNeutralDecodeNotice`: once a library holds a RAW, until it is dismissed once.
    pub(super) fn offer_neutral_decode_notice(&self, paths: &[String]) {
        let Some(w) = self.window() else { return };
        let seen = Preferences::load(&self.state.borrow().support).bool(preferences::NEUTRAL_DECODE_NOTICE_SHOWN).unwrap_or(false);
        if seen || self.catching_up() || crate::bench::requested().is_some() || !paths.iter().any(|p| oni_decode::is_raw(Path::new(p))) {
            return;
        }
        w.global::<crate::Notice>().set_shown(true);
    }

    /// Marked seen once dismissed, never on asking: a notice under another sheet would be spent unseen.
    pub(super) fn neutral_decode_notice_dismissed(&self) {
        if let Some(w) = self.window() {
            w.global::<crate::Notice>().set_shown(false);
        }
        let support = self.state.borrow().support.clone();
        if let Err(e) = Preferences::load(&support).set(preferences::NEUTRAL_DECODE_NOTICE_SHOWN, true.into()) {
            self.say(&format!("The notice cannot be marked seen: {e}"));
        }
    }

    pub(super) fn publish_tree(&self) {
        let Some(w) = self.window() else { return };
        self.unfold_archive();
        let beside_photo = !w.global::<App>().get_gallery();
        let mut guard = self.state.borrow_mut();
        let s = &mut *guard;
        let Some(archive) = &s.archive else { return };
        let rows = archive.rows();
        let ids: Vec<oni_library::archive_tree::NodeId> = rows.iter().map(|r| r.id.clone()).collect();
        tree_rename::land(&mut s.rename_request, &mut s.tree_renaming, &ids, beside_photo);
        let paths: Vec<Option<String>> = rows
            .iter()
            .map(|r| match &r.id {
                oni_library::archive_tree::NodeId::Photo(p) => Some(p.clone()),
                _ => None,
            })
            .collect();
        let mut rows: Vec<TreeRow> = rows
            .into_iter()
            .zip(&paths)
            .map(|(r, path)| {
                let renamed = match &r.id {
                    oni_library::archive_tree::NodeId::Folder(p) if s.tree_renaming.as_deref() == Some(p.as_str()) => Some(oni_library::paths::name(p)),
                    _ => None,
                };
                TreeRow {
                    depth: r.depth as i32,
                    // `explicitName`: a name typed over is the folder's own, never its roll's.
                    renaming: renamed.is_some(),
                    title: renamed.unwrap_or(r.title).into(),
                    caption: r.caption.into(),
                    kind: r.kind,
                    expandable: r.expandable,
                    expanded: r.expanded,
                    selected: path.as_deref().is_some_and(|p| s.tree_marks(p)),
                    ..TreeRow::default()
                }
            })
            .collect();
        s.tree_paths = paths;
        drop(guard);
        self.lay_tree(&mut rows);
        w.global::<Library>().set_rows(ModelRc::new(VecModel::from(rows)));
        self.follow_tree();
    }

    /// The tree's rows relit after a pick, in place: the list keeps its rows and its scroll.
    pub(super) fn mark_tree(&self) {
        let Some(w) = self.window() else { return };
        let model = w.global::<Library>().get_rows();
        let s = self.state.borrow();
        if model.row_count() != s.tree_paths.len() {
            drop(s);
            return self.publish_tree();
        }
        for (i, path) in s.tree_paths.iter().enumerate() {
            let marked = path.as_deref().is_some_and(|p| s.tree_marks(p));
            if let Some(mut row) = model.row_data(i).filter(|r| r.selected != marked) {
                row.selected = marked;
                model.set_row_data(i, row);
            }
        }
    }
}
