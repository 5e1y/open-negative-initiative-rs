//! Wiring the navigator, and what it keeps across launches: the sections folded, the unfolded rows, the height.

use super::*;

impl Ctl {
    pub(in crate::app) fn wire_navigator(me: &Rc<Ctl>, window: &crate::MainWindow) {
        me.load_navigator();
        let n = window.global::<crate::Navigator>();
        let index = |i: i32| i.max(0) as usize;
        let p = me.clone();
        n.on_row_clicked(move |i| p.navigator_row_clicked(index(i)));
        let p = me.clone();
        n.on_row_folded(move |i| p.navigator_row_folded(index(i)));
        let p = me.clone();
        n.on_keys_taken(move || p.rows_taken());
        let p = me.clone();
        n.on_key(move |text, command| p.navigator_key(&text, command));
        let p = me.clone();
        n.on_star_clicked(move |i| {
            let path = p.state.borrow().nav.rows.get(index(i)).cloned();
            if let Some(path) = path {
                p.toggle_favourite(&path);
            }
        });
        let p = me.clone();
        n.on_row_menu(move |i| p.navigator_row_menu(index(i)));
        let p = me.clone();
        n.on_menu_chosen(move |i| p.navigator_menu_chosen(index(i)));
        let p = me.clone();
        n.on_rename_committed(move |i, draft| {
            p.rename_committed(index(i), &draft);
            p.keys_back();
        });
        let p = me.clone();
        n.on_rename_cancelled(move || {
            p.state.borrow_mut().nav.model.renaming = None;
            p.publish_navigator();
            p.keys_back();
        });
        let p = me.clone();
        n.on_height_stored(move |h| {
            let _ = p.state.borrow_mut().preferences.set(EDITOR_HEIGHT, Value::from(h as f64));
        });
        let p = me.clone();
        n.on_mode_changed(move || p.follow_open_photo());
        let p = me.clone();
        // Add, wherever it is written: the folder starred, then opened.
        n.on_welcome_added(move |i| {
            let folder = p.state.borrow().nav.welcome.get(index(i)).cloned();
            if let Some(folder) = folder {
                p.add_starred(&[std::path::PathBuf::from(&folder)]);
            }
        });
        let p = me.clone();
        n.on_welcome_chose(move || p.act(crate::Act::AddFolder));
        let p = me.clone();
        n.on_catch_up_chose(move || p.act(crate::Act::AddFolder));
        let p = me.clone();
        n.on_catch_up_added(move |i| {
            let folder = p.state.borrow().nav.catch_up_folders.get(index(i)).cloned();
            if let Some(folder) = folder {
                p.add_starred(&[std::path::PathBuf::from(&folder)]);
            }
        });
        let p = me.clone();
        n.on_catch_up_done(move || p.catch_up_done());
        Ctl::watch_volumes(me);
        // The ONI Trash stands in every view from the launch, before any folder is laid.
        me.publish_trash_rows();
    }

    /// `FolderNavigation.init` and the launch's favourites: the remembered sections, rows and height;
    /// the places file, seeded once from `roots.json`; a former library's folders; the launch's notice.
    pub(super) fn load_navigator(&self) {
        let (height, catching_up, failure) = {
            let mut s = self.state.borrow_mut();
            let prefs = &s.preferences;
            let strings = |key: &str| -> BTreeSet<String> {
                prefs.value(key).and_then(Value::as_array).map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect()).unwrap_or_default()
            };
            let (folded, unfolded, favourites) = (strings(FOLDED), strings(UNFOLDED), strings(UNFOLDED_FAVOURITES));
            let height = prefs.value(EDITOR_HEIGHT).and_then(Value::as_f64).unwrap_or(EDITOR_REST);
            let remembered_opened = prefs.string(crate::preferences::OPENED_FOLDER).map(str::to_owned);
            let caught_up = prefs.bool(ROOTS_CATCH_UP_SHOWN).unwrap_or(false);
            // The home alone until the volumes' thread lists the rest: a sleeping mount never holds the launch.
            let volumes: Vec<String> = Some(s.nav.computer.home.clone()).filter(|h| !h.is_empty()).into_iter().collect();
            s.nav.model = Navigation::new(folded, unfolded, favourites, volumes);
            s.nav.remembered = Some(s.nav.model.remembered());
            s.org.trash_unfolded = s.nav.model.unfolded.contains(TRASH_ROW);
            let roots = Roots::load(&s.support);
            let failure = match Places::load(&s.support) {
                // Their import is due: shown as the old files hold them, written over by nothing.
                _ if s.nav.brought.places_due => {
                    s.nav.favourites = oni_library::migrate::places_held(&s.support);
                    None
                }
                Some(read) => {
                    s.nav.favourites = read;
                    if !s.nav.favourites.is_newer() {
                        s.record_places();
                    }
                    None
                }
                None => {
                    s.nav.favourites = Places::seeded(&roots);
                    s.save_favourites()
                }
            };
            let former = oni_library::library::load(&s.support).files;
            let opened = remembered_opened.or_else(|| roots.current().map(str::to_owned));
            if s.nav.favourites.places.is_empty() && opened.is_none() {
                s.nav.welcome = roots::suggested(&former, &s.nav.computer.home);
            }
            // A bench never meets the sheet, as Swift's `--bench` leaves before it.
            s.nav.catching_up = !former.is_empty() && !caught_up && crate::bench::requested().is_none();
            s.nav.former = former;
            (height, s.nav.catching_up, failure)
        };
        if let Some(message) = failure {
            self.say(&message);
        }
        let notice = self.state.borrow().nav.brought.notice;
        if let Some(notice) = notice {
            self.told(tr("Favourites and Projects"), tr(notice));
        }
        if let Some(w) = self.window() {
            let n = w.global::<crate::Navigator>();
            n.set_height(height as f32);
            n.set_catch_up_shown(catching_up);
        }
        self.publish_navigator();
    }

    /// Each favourite's volume and file id as read off the interface thread, written when one changed;
    /// a support folder removed meanwhile, a check's own, is not made again.
    pub(super) fn places_noted(&self, noted: Places) {
        let mut s = self.state.borrow_mut();
        if s.nav.favourites.is_newer() || s.nav.brought.places_due || !s.support.is_dir() {
            return;
        }
        if s.nav.favourites.take_identities(&noted) {
            let _ = s.nav.favourites.save(&s.support);
        }
    }

    /// Reads the mounted volumes off this thread, first at once, then whenever Computer's rows
    /// change, and the folders the disk refused once they answer: a drive waking or access granted.
    pub(super) fn watch_volumes(me: &Rc<Ctl>) {
        let (token, computer, refused) = {
            let s = me.state.borrow();
            (s.nav.watcher.clone(), s.nav.computer.clone(), s.nav.refused.clone())
        };
        let _ = std::thread::Builder::new().name("volumes".into()).spawn(move || {
            let mut seen: Option<Vec<String>> = None;
            loop {
                if Arc::strong_count(&token) <= 1 {
                    return;
                }
                let answering = answering(&refused, |f| std::fs::read_dir(f).is_ok());
                if !answering.is_empty() {
                    refused.lock().expect("refused").retain(|f| !answering.contains(f));
                }
                let now = computer.computer();
                let first = seen.is_none();
                if first || seen.as_ref() != Some(&now) || !answering.is_empty() {
                    let volumes = now.clone();
                    with_shell(move |c| c.volumes_moved(volumes, answering, first));
                }
                seen = Some(now);
                std::thread::sleep(MOUNTS);
            }
        });
    }

    /// The one answer to the disks moving: what left turns offline from its paths alone; on a volume
    /// back, the opened folder is read again once, its favourites too, and the kept edits written.
    pub(super) fn volumes_moved(self: &Rc<Self>, now: Vec<String>, answering: Vec<String>, first: bool) {
        let (moved, left, reread, arrived) = {
            let mut s = self.state.borrow_mut();
            let before = s.nav.model.set_volumes(now.clone());
            let moved = before != now;
            let (left, arrived): (Vec<String>, Vec<String>) = if first {
                (Vec::new(), Vec::new())
            } else {
                (before.iter().filter(|v| !now.contains(v)).cloned().collect(), now.iter().filter(|v| !before.contains(v)).cloned().collect())
            };
            let on = |vs: &[String], p: &str| vs.iter().any(|v| holds(v, p));
            s.org.left.retain(|v| !arrived.contains(v));
            s.org.left.extend(left.iter().cloned());
            presence::left(s.org.presence.iter_mut(), &left);
            presence::left(s.nav.presence.iter_mut(), &left);
            // What lies on a volume back is unknown until read or looked at again.
            s.org.presence.retain(|p, _| !on(&arrived, p));
            s.nav.presence.retain(|p, _| !on(&arrived, p));
            if !answering.is_empty() {
                s.nav.model.retry();
            }
            let opened = s.opened().map(str::to_owned);
            let refused = opened.as_ref().is_some_and(|o| answering.contains(o) && s.survey().unreadable.contains(o));
            let reread = refused || opened.as_ref().is_some_and(|o| on(&arrived, o));
            (moved, left, reread, !arrived.is_empty())
        };
        self.list_wanted();
        if reread {
            self.read_again();
        } else if arrived {
            self.probe_presence();
        }
        if !left.is_empty() {
            self.lay_grid();
        }
        if arrived {
            self.rewatch_anew();
        }
        self.publish_navigator();
        if moved {
            self.retry_unsaved(false);
        }
    }
}

/// The folders refused once that answer now, probed with the list copied out: a mount that hangs
/// never holds the lock the interface takes to publish the navigator.
fn answering(refused: &std::sync::Mutex<Vec<String>>, answers: impl Fn(&str) -> bool) -> Vec<String> {
    let listed = refused.lock().expect("refused").clone();
    listed.into_iter().filter(|f| answers(f)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn a_refused_folder_is_probed_with_the_lock_free() {
        let refused = Mutex::new(vec!["/a".to_string(), "/b".to_string()]);
        let free = std::cell::Cell::new(true);
        let probe = |f: &str| {
            free.set(free.get() && refused.try_lock().is_ok());
            f == "/b"
        };
        assert_eq!(answering(&refused, probe), ["/b"]);
        assert!(free.get(), "the interface could take the lock while a probe ran");
        // Twin: the list filtered under its guard holds the lock through every probe.
        free.set(true);
        let held: Vec<String> = refused.lock().unwrap().iter().filter(|f| probe(f)).cloned().collect();
        assert_eq!(held, ["/b"]);
        assert!(!free.get(), "a probe under the guard finds the lock taken");
    }
}
