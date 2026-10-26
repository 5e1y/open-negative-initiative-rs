//! Gestures on several photos: targets, pastes, the ONI Trash, the confirm sheet's answer.

use super::*;

mod busy;
mod disk;
mod told;

pub(in crate::app) use busy::wait_for_disk;

impl Ctl {
    /// `propagationTargets`: the selection, else beside it the open photo; in the gallery the
    /// selection alone, as the photo opened last may not even be on screen. The ONI Trash's never are.
    pub(super) fn targets(&self) -> Vec<String> {
        let gallery = self.window().is_some_and(|w| w.global::<App>().get_gallery());
        self.state.borrow().targets_in(gallery)
    }
}

impl State {
    pub(in crate::app) fn targets_in(&self, gallery: bool) -> Vec<String> {
        if self.picks.selection.is_empty() {
            return self.open.clone().filter(|_| !gallery).into_iter().collect();
        }
        self.selected()
    }

    /// How many photos a gesture reaches now, the bar's and the menus' count alike.
    pub(in crate::app) fn target_count(&self, gallery: bool) -> usize {
        match self.picks.selection.len() {
            0 => usize::from(self.open.is_some() && !gallery),
            n => n,
        }
    }
}

impl Ctl {

    /// A gesture on several photos through the core, then what it wrote read back: the rows, the
    /// open frame's sliders, the others' thumbnails.
    pub(super) fn gesture(self: &Rc<Self>, touched: &[String], run: impl FnOnce(&mut oni_core::EditorStore) -> usize) {
        self.gesture_as(tr("Something Went Wrong"), touched, run);
    }

    /// `gesture`, a failure told under the gesture's own name, as Swift's `reportFailure(_:as:)`.
    pub(super) fn gesture_as(self: &Rc<Self>, title: &str, touched: &[String], run: impl FnOnce(&mut oni_core::EditorStore) -> usize) {
        if self.busy_said(title) {
            return;
        }
        let failure = {
            let mut s = self.state.borrow_mut();
            let Some(store) = s.store.editor() else { return };
            run(store);
            store.failure.take()
        };
        if let Some(message) = failure {
            self.told(title, &message);
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

    /// `undo()`/`redo()` in the gallery: the gallery's own stack, whichever photos it touched; a
    /// roll's moves walk off this thread, and one walked back in part says how many items stayed.
    pub(super) fn gallery_step(self: &Rc<Self>, undo: bool) {
        let title = if undo { tr("Undo") } else { tr("Redo") };
        if self.busy_said(title) {
            return;
        }
        let (touched, moves): (Vec<String>, Vec<Move>) = {
            let mut s = self.state.borrow_mut();
            let Some(store) = s.store.editor() else { return };
            let history = store.gallery_history();
            let paths = if undo { history.undo_paths() } else { history.redo_paths() };
            let moves = if undo { history.undo_moves() } else { history.redo_moves() };
            (paths.into_iter().collect(), moves)
        };
        if !moves.is_empty() && self.unsaved_among(title, &moves) {
            return;
        }
        let (failure, walk) = {
            let mut s = self.state.borrow_mut();
            let Some(store) = s.store.editor() else { return };
            if undo { store.undo(Surface::Gallery) } else { store.redo(Surface::Gallery) };
            (store.failure.take(), store.walking.take())
        };
        if let Some(walk) = walk {
            return self.walk_off_thread(title, walk, touched);
        }
        if let Some(message) = failure {
            self.told(title, &message);
        }
        self.landed(&touched);
    }

    /// `resetAll()`: every target back to its imported state, one gallery step.
    pub(super) fn reset_all(self: &Rc<Self>) {
        let targets = self.targets();
        self.gesture(&targets, |store| store.reset_frames(&targets));
    }

    /// A mode change drops the split, as Swift's does.
    pub(super) fn drop_split(self: &Rc<Self>) {
        if self.state.borrow().compare.split.is_some() {
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
        self.commit_info_drafts();
        let ringed = {
            let s = self.state.borrow();
            s.picks.anchor.clone().filter(|_| !s.trash_shown())
        };
        let Some(path) = ringed else { return };
        self.drop_split();
        if self.state.borrow().open.as_deref() == Some(path.as_str()) {
            // As `open_in_editor`: no edit lands on a frame whose file a gesture may be moving.
            if self.busy_said(tr("Edit")) {
                return;
            }
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
            PasteCheck::AnotherRoll => self.confirm(tr("Paste Settings"), Pending::Paste(targets)),
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
            PasteCheck::AnotherRoll => self.confirm(tr("Paste to Roll"), Pending::PasteToRoll),
            _ => self.lay_paste_to_roll(),
        }
    }

    pub(super) fn lay_paste_to_roll(self: &Rc<Self>) {
        if self.busy_said(tr("Paste to Roll")) {
            return;
        }
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

    /// The marks written off this thread: a roll of three hundred on a slow disk never stalls the window.
    pub(super) fn move_paths_to_trash(self: &Rc<Self>, targets: &[String]) {
        self.mark_off_thread(tr("Move to ONI Trash"), targets.to_vec(), Some(oni_settings::Date::now()));
    }

    /// The film type's tabs: `applyMode` on the selection, else the open photo.
    pub(super) fn film_type(self: &Rc<Self>, index: i32) {
        let Some(mode) = usize::try_from(index).ok().and_then(|i| ConversionMode::ALL.get(i).copied()) else { return };
        let targets = self.targets();
        self.gesture(&targets, |store| store.apply_mode(mode, &targets));
    }

    /// `DSDialogRequest.confirm`: the sheet over the window until a key answers it.
    pub(super) fn confirm(&self, key: &str, pending: Pending) {
        self.ask(tr(ANOTHER_ROLL.0), tr(ANOTHER_ROLL.1), key, false, pending);
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
            Some(Pending::NewRoll(plan)) => self.lay_new_roll(*plan),
            Some(Pending::QuitAnyway) => {
                self.state.borrow_mut().quitting = true;
                let _ = slint::quit_event_loop();
            }
            Some(Pending::DownloadUpdate) => self.download_update(),
            Some(Pending::Locate(asked)) => self.locate_answered(asked),
            Some(Pending::Run(then)) => then(self),
            None => {}
        }
        self.publish_bar();
    }
}
