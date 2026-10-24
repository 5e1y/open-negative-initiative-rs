//! The archive: opening a folder, reading its rows, the tree, the thumbnails' pass and opening a photo.

use super::*;

mod adding;

impl Ctl {
    // MARK: - The archive

    /// `openFolder`: the gallery on the folder, read through the index behind, its thumbnails
    /// drawn once the read is whole; the editor keeps what it holds.
    pub(super) fn open_folder(self: &Rc<Self>, folder: &Path) {
        self.open_spelt(oni_library::paths::canonical(folder));
    }

    /// `open_folder` on a path already in the archive's spelling, which reads no disk here.
    pub(super) fn open_spelt(self: &Rc<Self>, folder: String) {
        let (generation, index_path) = {
            let mut s = self.state.borrow_mut();
            s.org.trash_open = false;
            s.org.renaming = None;
            s.org.presence.clear();
            s.org.opened_presence = oni_library::presence::Presence::Present;
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
        self.transition_check(&folder);
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
        let (current, computer) = {
            let mut s = self.state.borrow_mut();
            s.org.reads += 1;
            (s.generation.clone(), s.nav.computer.clone())
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
                    let survey = survey.clone();
                    with_shell(move |c| c.take_rows(generation, rows, done, survey));
                });
            })
            .expect("library thread");
    }

    /// `takeRows` then `layGallery`: the rows laid, the selection and the open photo kept by path.
    pub(super) fn take_rows(self: &Rc<Self>, generation: u64, rows: Vec<oni_library::Entry>, done: bool, survey: Arc<Survey>) {
        {
            let mut s = self.state.borrow_mut();
            if s.generation.load(Ordering::Relaxed) != generation {
                return;
            }
            s.copies = s.index.as_ref().map(LibraryIndex::copies).unwrap_or_default();
            s.fingerprints = rows.iter().map(|e| (e.path.clone(), s.tile_key(e))).collect();
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
        }
    }

    /// A roll marker this app just wrote, laid as the survey would have read it, with no read back.
    pub(super) fn roll_written(&self, folder: &str, film: Option<oni_settings::Film>) {
        let mut s = self.state.borrow_mut();
        let holding = s.survey.images.iter().any(|i| oni_library::paths::same(&oni_library::paths::parent(i), folder));
        let rolls = &mut Arc::make_mut(&mut s.survey).rolls;
        match film.filter(|_| holding) {
            Some(film) => rolls.insert(folder.to_owned(), film),
            None => rolls.remove(folder),
        };
    }

    /// The archive cut again from the rows held, under the filter; what the hand folded stays so,
    /// except in a folder just opened.
    pub(super) fn lay_archive(self: &Rc<Self>) {
        let Some(folder) = self.state.borrow().opened.clone() else {
            // The ONI Trash and the projects are the library's, laid with no folder open too.
            self.read_library();
            return self.lay_grid();
        };
        let (title, stock) = {
            let mut s = self.state.borrow_mut();
            let mut archive = Archive::from_rows(&folder, &s.rows, &s.survey, s.filter);
            let fresh = std::mem::take(&mut s.reveal.fresh);
            if let Some(old) = s.archive.as_ref().filter(|_| !fresh) {
                let folded = old.folded();
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
        self.read_library();
        self.lay_grid();
        self.publish_tree();
        self.publish_bar();
    }

    /// `fillOpened`: the pass over the opened folder's rows, which draws those with no file; a
    /// photo whose original cannot be reached is never decoded, only counted once if it lacks one.
    pub(super) fn fill(&self) {
        let s = self.state.borrow();
        let generation = s.generation.load(Ordering::Relaxed);
        let (rows, gone): (Vec<_>, Vec<_>) = s
            .rows
            .iter()
            // A file the cloud keeps away is drawn once opened, never downloaded by the pass.
            .filter(|e| !e.fingerprint.starts_with(thumbnails::PENDING) && !s.survey.cloud.contains(&e.path))
            .map(|e| (e.path.clone(), s.tile_key(e)))
            .partition(|(path, _)| s.presence(path).is_present());
        let gone = gone.into_iter().map(|(_, fingerprint)| fingerprint).collect();
        let (lock, wake) = &*s.painter;
        lock.lock().expect("painter").pass = Some((generation, rows, gone));
        wake.notify_one();
    }

    /// `invalidate`: the photo's redraw, queued once and drawn before the pass's next photo.
    pub(super) fn redraw(&self, path: &str) {
        let mut s = self.state.borrow_mut();
        if !s.presence(path).is_present() {
            return;
        }
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

    /// The pass's count once it knows which photos lack a file, and one line for those whose
    /// original cannot be reached, never one per photo.
    pub(super) fn pass_counted(&self, generation: u64, held: usize, absent: usize, unreachable: usize) {
        {
            let mut s = self.state.borrow_mut();
            if s.generation.load(Ordering::Relaxed) != generation {
                return;
            }
            s.painted = (held, held + absent);
            s.creating = (0, absent);
        }
        if unreachable > 0 {
            self.say(&trn("One photo has no thumbnail: its original cannot be reached.", "{n} photos have no thumbnail: their originals cannot be reached.", unreachable as u64));
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
                    s.thumbs.insert(fingerprint.clone(), image(&rgba, w, h));
                    None
                }
                Err(e) => {
                    s.failed.insert(fingerprint.clone());
                    Some(format!("{name}: {e}"))
                }
            }
        };
        if let Some(message) = failure {
            self.say(&message);
        }
        self.relay_thumbnail(&fingerprint);
        self.publish_tiles();
    }

    /// A file read from the folder; one absent stays asked, so only a render fills its tile.
    pub(super) fn disk_landed(&self, fingerprint: String, read: Option<(Vec<u8>, usize, usize)>) {
        let Some((rgba, w, h)) = read else { return };
        {
            let mut s = self.state.borrow_mut();
            s.asked.remove(&fingerprint);
            s.thumbs.insert(fingerprint.clone(), image(&rgba, w, h));
        }
        self.relay_thumbnail(&fingerprint);
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
            self.say(&tr!("The notice cannot be marked seen: {error}", error = e));
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
        let waiting = s.rename_request.is_some();
        tree_rename::land(&mut s.rename_request, &mut s.tree_renaming, &ids, beside_photo);
        // `requestLanded` reveals the row it types over.
        let landed = (waiting && s.rename_request.is_none())
            .then(|| ids.iter().position(|id| matches!(id, oni_library::archive_tree::NodeId::Folder(p) if s.tree_renaming.as_deref() == Some(p.as_str()))))
            .flatten();
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
                let title = renamed.clone().unwrap_or(r.title);
                let (head, tail) = oni_library::navigator::split_end(&title);
                TreeRow {
                    depth: r.depth as i32,
                    // `explicitName`: a name typed over is the folder's own, never its roll's.
                    renaming: renamed.is_some(),
                    title: title.into(),
                    head: head.into(),
                    tail: tail.into(),
                    caption: r.caption.into(),
                    kind: r.kind,
                    stock: r.stock.and_then(crate::stocks::icon).unwrap_or_default(),
                    has_stock: r.stock.and_then(crate::stocks::icon).is_some(),
                    expandable: r.expandable,
                    expanded: r.expanded,
                    selected: path.as_deref().is_some_and(|p| s.tree_marks(p)) || s.org.tree_pick.selection.contains(&r.id),
                    ..TreeRow::default()
                }
            })
            .collect();
        s.org.tree_ids = ids;
        drop(guard);
        self.lay_column(tree_column::Which::Archive, &mut rows, paths);
        w.global::<Library>().set_rows(ModelRc::new(VecModel::from(rows)));
        self.follow_tree();
        if let Some(i) = landed {
            self.centre_row(tree_column::Which::Archive, i);
        }
        self.pin_column(tree_column::Which::Archive);
    }

    /// The tree's rows relit after a pick, in place: the list keeps its rows and its scroll.
    pub(super) fn mark_tree(&self) {
        let Some(w) = self.window() else { return };
        let model = w.global::<Library>().get_rows();
        let s = self.state.borrow();
        if model.row_count() != s.tree.paths.len() {
            drop(s);
            return self.publish_tree();
        }
        for (i, path) in s.tree.paths.iter().enumerate() {
            let picked = s.org.tree_ids.get(i).is_some_and(|id| s.org.tree_pick.selection.contains(id));
            let marked = picked || path.as_deref().is_some_and(|p| s.tree_marks(p));
            if let Some(mut row) = model.row_data(i).filter(|r| r.selected != marked) {
                row.selected = marked;
                model.set_row_data(i, row);
            }
        }
        drop(s);
        self.pin_column(tree_column::Which::Archive);
    }
}
