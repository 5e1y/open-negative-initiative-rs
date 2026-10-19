//! Reading the folders off this thread, the favourites after the opened one, and following the open photo.

use super::*;

impl Ctl {
    /// `openFolder`'s `surveys.yield`: a favourite's read gives way to the folder opened.
    pub(in crate::app) fn navigator_opened(&self, folder: &str) {
        {
            let mut s = self.state.borrow_mut();
            s.nav.surveys.yield_keeping(&[folder.to_owned()]);
            s.nav.waiting = true;
        }
        self.publish_navigator();
    }

    /// The opened folder read whole: its rolls known, then the favourites read in turn.
    pub(in crate::app) fn navigator_read(&self) {
        {
            let mut s = self.state.borrow_mut();
            let rolls: HashSet<String> = folders::rolls(&s.survey.images).into_keys().collect();
            if let Some(opened) = s.opened.clone() {
                s.nav.roll_edits.retain(|k, _| !holds(&opened, k));
            }
            s.nav.opened_rolls = rolls;
            s.nav.waiting = false;
        }
        self.start_pass();
        self.publish_navigator();
    }

    /// Folders the disk refused now answer: listed again, and the opened one read again.
    pub(super) fn answering(self: &Rc<Self>, folders: &[String]) {
        let reopen = {
            let mut s = self.state.borrow_mut();
            s.nav.model.retry();
            s.opened.clone().filter(|o| folders.contains(o) && s.survey.unreadable.contains(o))
        };
        self.list_wanted();
        if let Some(folder) = reopen {
            self.open_folder(Path::new(&folder));
        }
    }

    pub(super) fn start_pass(&self) {
        self.state.borrow_mut().nav.tried.clear();
        self.survey_favourites();
    }

    /// `surveyFolders` for the favourites: each one no read area covers, read alone off this
    /// thread, the folders holding the opened one first.
    pub(super) fn survey_favourites(&self) {
        let (root, stop) = {
            let mut s = self.state.borrow_mut();
            let wanted = s.wanted_folders();
            s.nav.surveys.keep(&wanted);
            if s.nav.reading || s.nav.waiting {
                return;
            }
            let opened = s.opened.clone();
            let mut stored: Vec<String> = s.nav.surveys.areas.keys().cloned().collect();
            stored.extend(opened.iter().cloned());
            stored.extend(s.nav.tried.iter().cloned());
            let missing = favourites::missing(&s.nav.favourites.paths, &stored);
            let next = favourites::ordered(&missing, opened.as_deref()).into_iter().find(|r| Some(r) != opened.as_ref());
            let Some(root) = next else { return };
            s.nav.tried.insert(root.clone());
            s.nav.reading = true;
            let stop = s.nav.surveys.begin(&root);
            (root, stop)
        };
        let _ = std::thread::Builder::new().name("favourites".into()).spawn(move || {
            let area = favourites::read(&root, &stop);
            with_shell(move |c| c.favourite_read(root, area));
        });
    }

    pub(super) fn favourite_read(&self, root: String, area: Option<Area>) {
        {
            let mut s = self.state.borrow_mut();
            s.nav.surveys.end();
            s.nav.reading = false;
            let wanted = s.wanted_folders();
            if let Some(area) = area.filter(|_| wanted.iter().any(|w| holds(&root, w) || holds(w, &root))) {
                s.nav.roll_edits.retain(|k, _| !holds(&root, k));
                s.nav.surveys.store(area, &root);
            }
            let inside = |p: &str| wanted.iter().any(|w| holds(w, p));
            let rolls: HashSet<String> =
                s.nav.surveys.areas.values().flat_map(|a| a.rolls.keys()).filter(|k| inside(k)).cloned().collect();
            s.nav.area_rolls = rolls;
        }
        self.publish_navigator();
        self.survey_favourites();
    }

    /// `reveal(current)`: beside a photo, the navigator unfolds down to the photo's folder and
    /// scrolls it to the centre.
    pub(in crate::app) fn follow_open_photo(&self) {
        let Some(w) = self.window() else { return };
        let gallery = w.global::<App>().get_gallery();
        {
            let mut s = self.state.borrow_mut();
            let current = if gallery { None } else { s.open.as_deref().map(paths::parent) };
            if current == s.nav.current {
                return;
            }
            s.nav.current = current.clone();
            let Some(folder) = current else { return };
            let favourites = s.nav.favourites.paths.clone();
            let computer = s.nav.computer.clone();
            s.nav.model.show(&folder, &favourites, &computer);
        }
        self.publish_navigator();
        let row = {
            let s = self.state.borrow();
            s.nav.current.as_ref().and_then(|c| s.nav.rows.iter().position(|r| r == c))
        };
        if let Some(row) = row {
            let n = w.global::<crate::Navigator>();
            n.set_reveal_row(row as i32);
            n.set_reveal_serial(n.get_reveal_serial() + 1);
        }
    }

    /// The ONI Trash's fold and pick, held by the navigator as Swift's `unfolded` and `selected`.
    pub(in crate::app) fn trash_synced(&self) {
        let moved = {
            let mut s = self.state.borrow_mut();
            let unfolded = s.org.trash_unfolded;
            if unfolded != s.nav.model.unfolded.contains(TRASH_ROW) {
                if unfolded {
                    s.nav.model.unfolded.insert(TRASH_ROW.to_owned());
                } else {
                    s.nav.model.unfolded.remove(TRASH_ROW);
                }
            }
            let picked = s.org.trash_picked.clone().map(|p| format!("{TRASH_ROW}{p}"));
            let moved = picked.is_some() && picked != s.nav.model.selected;
            if moved {
                s.nav.model.selected = picked;
            }
            s.remember();
            moved
        };
        if moved {
            self.publish_navigator();
        }
    }
}
