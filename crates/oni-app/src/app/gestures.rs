//! Gestures on several photos: targets, pastes, new rolls, the ONI Trash, the confirm sheet's answer.

use super::*;

impl Ctl {
    /// `propagationTargets`: the selection, else the open photo; the ONI Trash's never are.
    pub(super) fn targets(&self) -> Vec<String> {
        let s = self.state.borrow();
        if s.picks.selection.is_empty() {
            return s.open.clone().into_iter().collect();
        }
        s.selected()
    }

    /// A gesture on several photos through the core, then what it wrote read back: the rows, the
    /// open frame's sliders, the others' thumbnails.
    pub(super) fn gesture(self: &Rc<Self>, touched: &[String], run: impl FnOnce(&mut oni_core::EditorStore) -> usize) {
        let failure = {
            let mut s = self.state.borrow_mut();
            let Some(store) = s.store.editor() else { return };
            run(store);
            store.failure.take()
        };
        if let Some(message) = failure {
            self.say(&message);
        }
        self.landed(touched);
    }

    pub(super) fn landed(self: &Rc<Self>, touched: &[String]) {
        self.closed_if_trashed();
        self.publish(false);
        self.reread(touched);
        let open = self.state.borrow().open.clone();
        for path in touched.iter().filter(|p| Some(*p) != open.as_ref()) {
            self.redraw(path);
        }
    }

    /// `took(_:at:)`: a trashed open frame leaves the editor, which falls back on the gallery.
    pub(super) fn closed_if_trashed(&self) {
        let closed = {
            let mut s = self.state.borrow_mut();
            let held = s.store.editor().map(|e| e.path().map(str::to_string));
            match held {
                Some(None) if s.open.is_some() => {
                    s.open = None;
                    s.negative = None;
                    true
                }
                _ => false,
            }
        };
        if closed && let Some(w) = self.window() {
            w.global::<App>().set_gallery(true);
            self.blank_canvas();
        }
    }

    /// `undo()`/`redo()` in the gallery: the gallery's own stack, whichever photos it touched.
    pub(super) fn gallery_step(self: &Rc<Self>, undo: bool) {
        let (touched, moves): (Vec<String>, Vec<Move>) = {
            let mut s = self.state.borrow_mut();
            let Some(store) = s.store.editor() else { return };
            let history = store.gallery_history();
            let paths = if undo { history.undo_paths() } else { history.redo_paths() };
            let moves = if undo { history.undo_moves() } else { history.redo_moves() };
            (paths.into_iter().collect(), moves)
        };
        if !moves.is_empty() {
            // The open frame's pending write lands before its file moves.
            let failure = self.state.borrow_mut().store.flush();
            if let Some(message) = failure {
                self.say(&message);
            }
        }
        self.gesture(&touched, |store| {
            if undo { store.undo(Surface::Gallery) } else { store.redo(Surface::Gallery) };
            0
        });
        if !moves.is_empty() {
            let made: Vec<Move> = moves.into_iter().filter(|m| Path::new(&m.to).exists() && !Path::new(&m.from).exists()).collect();
            self.followed(&made);
        }
    }

    /// `rollFolder(from:)`: a dated folder beside the first target, the photos, their passes and
    /// sidecars moved in, one gallery step that ⌘Z walks back.
    pub(super) fn new_roll(self: &Rc<Self>) {
        let targets: HashSet<String> = self.targets().into_iter().collect();
        self.new_roll_from(&targets);
    }

    /// `rollFolder(from:)`: the photos moved into a dated roll folder beside the first, one step.
    pub(super) fn new_roll_from(self: &Rc<Self>, targets: &HashSet<String>) {
        let (ordered, earliest) = {
            let s = self.state.borrow();
            let ordered: Vec<String> = s.order.iter().filter(|p| targets.contains(*p)).cloned().collect();
            let dates = s.rows.iter().filter(|e| targets.contains(&e.path)).filter_map(|e| e.captured_at);
            (ordered, dates.reduce(|a, b| if b < a { b } else { a }))
        };
        let Some(first) = ordered.first() else { return };
        let parent = oni_library::paths::parent(first);
        let name = oni_library::roll_folder::name_for(earliest.unwrap_or_else(oni_settings::Date::now), &parent);
        let film = oni_settings::Film::new(&name);
        let folder = match oni_library::roll_folder::make(&name, &parent, &film) {
            Ok(folder) => folder,
            Err(e) => return self.say(&format!("The roll's folder could not be made: {e}")),
        };
        let files = oni_library::rgb_passes::expanded(&ordered);
        let failure = self.state.borrow_mut().store.flush();
        if let Some(message) = failure {
            self.say(&message);
        }
        let made = oni_library::roll_folder::move_items(&oni_library::roll_folder::moves(&files, &folder));
        if made.len() < files.len() {
            self.say(&format!("{} of the files could not be moved into the roll.", files.len() - made.len()));
        }
        {
            let mut s = self.state.borrow_mut();
            if let Some(archive) = s.archive.as_mut() {
                archive.expanded.insert(oni_library::archive_tree::NodeId::Folder(parent.clone()));
                archive.expanded.insert(oni_library::archive_tree::NodeId::Folder(folder.clone()));
            }
            s.rename_request = Some(folder.clone());
            if let Some(store) = s.store.editor() {
                store.record_disk(oni_core::store::Disk { folder: Some(folder), film: Some(film), moves: made.clone() }, "New Roll");
            }
        }
        self.journal_count(oni_library::journal::Kind::Rolled, targets.len());

        self.followed(&made);
    }

    /// `followed(_:)`: the index rows, the selection and the open frame go where the disk moved
    /// them, then the folder is read again for its new tree.
    pub(super) fn followed(self: &Rc<Self>, moves: &[Move]) {
        let moved = |path: &str| {
            moves.iter().find_map(|m| {
                if path == m.from {
                    Some(m.to.clone())
                } else {
                    oni_library::paths::holds(&m.from, path).then(|| format!("{}{}", m.to, &path[m.from.len()..]))
                }
            })
        };
        let reopen = {
            let mut s = self.state.borrow_mut();
            if let Some(index) = &s.index {
                for row in index.all() {
                    if let Some(to) = moved(&row.path) {
                        let _ = index.remove(&row.path);
                        let _ = index.upsert(&oni_library::Entry { path: to, ..row });
                    }
                }
            }
            for row in &mut s.rows {
                if let Some(to) = moved(&row.path) {
                    row.path = to;
                }
            }
            let fingerprints = s.rows.iter().map(|e| (e.path.clone(), e.fingerprint.clone())).collect();
            s.fingerprints = fingerprints;
            for path in &mut s.order {
                if let Some(to) = moved(path) {
                    *path = to;
                }
            }
            s.open.as_deref().and_then(|p| moved(p))
        };
        self.lay_archive();
        if let Some(to) = reopen {
            self.open_photo(&to);
        }
        self.read_again();
    }

    /// `resetAll()`: every target back to its imported state, one gallery step.
    pub(super) fn reset_all(self: &Rc<Self>) {
        let targets = self.targets();
        self.gesture(&targets, |store| store.reset_frames(&targets));
    }

    /// A mode change drops the split, as Swift's does.
    pub(super) fn drop_split(self: &Rc<Self>) {
        if self.state.borrow().split.is_some() {
            self.toggle_compare();
        }
    }

    /// Back in the gallery, the editor's full-resolution frame and its copies are let go: up to 1.6 GB
    /// on a 24 MP frame; the view builds it again once it asks.
    fn leave_editor_memory(&self) {
        self.leave_together();
        {
            let mut s = self.state.borrow_mut();
            s.full = None;
            s.building_full = None;
        }
        if let Ok(mut engine) = self.engine.lock() {
            engine.trim();
        }
    }

    /// `toggleMode()`: the editor on the gallery's ringed photo, or back to the gallery.
    pub(super) fn toggle_mode(self: &Rc<Self>) {
        let Some(w) = self.window() else { return };
        // Leaving the editor takes the deck away with no exit event: the preview goes with it.
        self.clear_hover();
        if !w.global::<App>().get_gallery() {
            self.drop_split();
            w.global::<App>().set_gallery(true);
            self.leave_editor_memory();
            self.publish_tiles();
            return;
        }
        let ringed = {
            let s = self.state.borrow();
            s.picks.anchor.clone().filter(|_| !s.trash_shown())
        };
        let Some(path) = ringed else { return };
        self.drop_split();
        if self.state.borrow().open.as_deref() == Some(path.as_str()) {
            w.global::<App>().set_gallery(false);
            self.land_tree_request();
            // A selection of several made in the gallery shows together as the editor comes back.
            self.follow_selection();
        } else {
            self.open_in_editor(&path);
        }
    }

    /// nil, bronze, silver, gold, then nil again: exactly one medal, never a tier above.
    pub(super) fn cycle_filter(self: &Rc<Self>) {
        {
            let mut s = self.state.borrow_mut();
            s.filter = match s.filter {
                None => Some(Medal::Bronze),
                Some(Medal::Bronze) => Some(Medal::Silver),
                Some(Medal::Silver) => Some(Medal::Gold),
                _ => None,
            };
        }
        self.lay_archive();
    }

    /// `centreSelected()`: the open photo, else the first selected, scrolled into view.
    pub(super) fn show_selected(self: &Rc<Self>) {
        {
            let mut s = self.state.borrow_mut();
            let at = s.open.as_ref().and_then(|p| s.order.iter().position(|o| o == p)).or_else(|| s.order.iter().position(|o| s.picks.selection.contains(o)));
            let Some(at) = at else { return };
            if let Some(top) = s.layout.reveal(at, s.scroll, s.grid.1) {
                s.scroll = top;
            }
        }
        self.publish_tiles();
    }

    /// `pasteSettings(to:)`: one roll's look laid on another asks first.
    pub(super) fn paste(self: &Rc<Self>) {
        self.board_in();
        let targets = self.targets();
        let check = {
            let mut s = self.state.borrow_mut();
            let Some(store) = s.store.editor() else { return };
            store.paste_check(&targets)
        };
        match check {
            PasteCheck::Nothing => {}
            PasteCheck::Ready => self.lay_paste(targets),
            PasteCheck::AnotherRoll => self.confirm("Paste Settings", Pending::Paste(targets)),
        }
    }

    /// The journal counts the photos the paste changed, as Swift's `before.count`.
    pub(super) fn lay_paste(self: &Rc<Self>, targets: Vec<String>) {
        let changed = std::cell::Cell::new(0);
        self.gesture(&targets, |store| {
            changed.set(store.paste_settings(&targets));
            changed.get()
        });
        self.journal_count(oni_library::journal::Kind::Pasted, changed.get());
    }

    /// `pasteToRoll()`: a frame measured with no roll takes the paste alone; another roll's look asks first.
    pub(super) fn paste_to_roll(self: &Rc<Self>) {
        self.board_in();
        let check = {
            let mut s = self.state.borrow_mut();
            let Some(open) = s.open.clone() else { return };
            let roll = s.store.settings().roll_reference.is_some();
            let Some(store) = s.store.editor() else { return };
            if !store.has_copied_settings() {
                return;
            }
            if roll { store.paste_check(&[open]) } else { PasteCheck::Ready }
        };
        match check {
            PasteCheck::AnotherRoll => self.confirm("Paste to Roll", Pending::PasteToRoll),
            _ => self.lay_paste_to_roll(),
        }
    }

    pub(super) fn lay_paste_to_roll(self: &Rc<Self>) {
        let touched: Vec<String> = {
            let mut s = self.state.borrow_mut();
            let open = s.open.clone();
            let id = s.store.settings().roll_reference.as_ref().map(|r| r.id);
            let Some(store) = s.store.editor() else { return };
            store.paste_to_roll();
            let mut touched: Vec<String> = id.and_then(|id| store.measured_group(id)).map(|g| g.iter().cloned().collect()).unwrap_or_default();
            // `rollReach + 1`: the open frame counted once, whether or not its roll lists it.
            touched.retain(|p| Some(p) != open.as_ref());
            touched.extend(open);
            touched
        };
        self.journal_count(oni_library::journal::Kind::Pasted, touched.len());
        self.gesture(&touched, |_| 0);
    }

    /// `pasteFramingToRoll()`: the open frame first, then every frame measured with its roll.
    pub(super) fn paste_framing_to_roll(self: &Rc<Self>) {
        let targets: Vec<String> = {
            let mut s = self.state.borrow_mut();
            let Some(open) = s.open.clone() else { return };
            let id = s.store.settings().roll_reference.as_ref().map(|r| r.id);
            let Some(store) = s.store.editor() else { return };
            let members = id.map(|id| store.measured_with(id)).unwrap_or_default();
            std::iter::once(open.clone()).chain(members.into_iter().filter(|m| *m != open)).collect()
        };
        let size = self.image_size();
        self.gesture(&targets, |store| store.paste_framing(&targets, size));
    }

    /// `moveToOniTrash()`: each target's sidecar marked, its files untouched, one gallery step.
    pub(super) fn move_to_trash(self: &Rc<Self>) {
        let targets = self.targets();
        self.move_paths_to_trash(&targets);
    }

    pub(super) fn move_paths_to_trash(self: &Rc<Self>, targets: &[String]) {
        let targets = targets.to_vec();
        if targets.is_empty() {
            return;
        }
        self.gesture(&targets, |store| store.move_to_trash(&targets));
    }

    /// The film type's tabs: `applyMode` on the selection, else the open photo.
    pub(super) fn film_type(self: &Rc<Self>, index: i32) {
        let Some(mode) = usize::try_from(index).ok().and_then(|i| ConversionMode::ALL.get(i).copied()) else { return };
        let targets = self.targets();
        self.gesture(&targets, |store| store.apply_mode(mode, &targets));
    }

    /// `DSDialogRequest.confirm`: the sheet over the window until a key answers it.
    pub(super) fn confirm(&self, key: &str, pending: Pending) {
        self.ask(ANOTHER_ROLL.0, ANOTHER_ROLL.1, key, false, pending);
    }

    pub(super) fn answered(self: &Rc<Self>, go: bool) {
        if let Some(w) = self.window() {
            w.global::<Confirm>().set_shown(false);
        }
        let pending = self.state.borrow_mut().pending.take();
        match pending.filter(|_| go) {
            Some(Pending::Paste(targets)) => self.lay_paste(targets),
            Some(Pending::PasteToRoll) => self.lay_paste_to_roll(),
            Some(Pending::DeleteProject(id)) => self.delete_project(id),
            Some(Pending::RestoreAll) => self.restore_all(),
            Some(Pending::DeletePreset(id)) => self.delete_preset(id),
            Some(Pending::UpdatePreset(id)) => self.update_preset(id),
            Some(Pending::ResetToDefault(reach)) => self.reset_to_default(reach),
            Some(Pending::QuitAnyway) => {
                self.state.borrow_mut().quitting = true;
                let _ = slint::quit_event_loop();
            }
            Some(Pending::DownloadUpdate) => self.download_update(),
            None => {}
        }
        self.publish_bar();
    }
}
