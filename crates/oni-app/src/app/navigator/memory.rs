//! Wiring the navigator, and what it keeps across launches: base, favourites view, unfolded rows, height.

use super::*;

impl Ctl {
    pub(in crate::app) fn wire_navigator(me: &Rc<Ctl>, window: &crate::MainWindow) {
        me.load_navigator();
        let n = window.global::<crate::Navigator>();
        let index = |i: i32| i.max(0) as usize;
        let p = me.clone();
        n.on_row_clicked(move |i| p.navigator_row_clicked(index(i)));
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
        n.on_rename_committed(move |i, draft| p.rename_committed(index(i), &draft));
        let p = me.clone();
        n.on_rename_cancelled(move || {
            p.state.borrow_mut().nav.model.renaming = None;
            p.publish_navigator();
        });
        let p = me.clone();
        n.on_go_up(move || {
            {
                let mut s = p.state.borrow_mut();
                let computer = s.nav.computer.clone();
                s.nav.model.go_up(&computer);
            }
            p.publish_navigator();
        });
        let p = me.clone();
        n.on_favourites_toggled(move || {
            {
                let mut s = p.state.borrow_mut();
                let on = !s.nav.model.favourites_only;
                s.nav.model.show_favourites(on);
            }
            p.publish_navigator();
        });
        let p = me.clone();
        n.on_ancestor_chosen(move |i| p.ancestor_chosen(index(i)));
        let p = me.clone();
        n.on_height_stored(move |h| {
            let _ = p.state.borrow_mut().preferences.set(EDITOR_HEIGHT, Value::from(h as f64));
        });
        let p = me.clone();
        n.on_mode_changed(move || p.follow_open_photo());
        let p = me.clone();
        n.on_welcome_added(move |i| {
            let folder = p.state.borrow().nav.welcome.get(index(i)).cloned();
            if let Some(folder) = folder {
                p.add(&[std::path::PathBuf::from(&folder)]);
            }
        });
        let p = me.clone();
        n.on_welcome_chose(move || p.act(crate::Act::Open));
        let p = me.clone();
        n.on_catch_up_chose(move || p.act(crate::Act::Open));
        let p = me.clone();
        n.on_catch_up_added(move |i| {
            let folder = p.state.borrow().nav.catch_up_folders.get(index(i)).cloned();
            if let Some(folder) = folder {
                p.toggle_favourite(&folder);
                p.open_folder(Path::new(&folder));
            }
        });
        let p = me.clone();
        n.on_catch_up_done(move || p.catch_up_done());
        Ctl::watch_volumes(me);
        me.start_pass();
    }

    /// `FolderNavigation.init` and the launch's favourites: the remembered base, view, rows and
    /// height; `favourites.json`, seeded once from `roots.json`; a former library's folders.
    pub(super) fn load_navigator(&self) {
        let (height, catching_up, failure) = {
            let mut s = self.state.borrow_mut();
            let prefs = &s.preferences;
            let base = prefs.string(BASE).map(str::to_owned);
            let favourites_only = prefs.bool(FAVOURITES_ONLY).unwrap_or(false);
            let unfolded: BTreeSet<String> = prefs
                .value(UNFOLDED)
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
                .unwrap_or_default();
            let height = prefs.value(EDITOR_HEIGHT).and_then(Value::as_f64).unwrap_or(EDITOR_REST);
            let remembered_opened = prefs.string(crate::preferences::OPENED_FOLDER).map(str::to_owned);
            let caught_up = prefs.bool(ROOTS_CATCH_UP_SHOWN).unwrap_or(false);
            // The home alone until the volumes' thread lists the rest: a sleeping mount never holds the launch.
            let volumes: Vec<String> = Some(s.nav.computer.home.clone()).filter(|h| !h.is_empty()).into_iter().collect();
            s.nav.model = Navigation::new(base, favourites_only, unfolded, volumes);
            s.nav.remembered = Some(s.nav.model.remembered());
            s.org.trash_unfolded = s.nav.model.unfolded.contains(TRASH_ROW);
            let roots = Roots::load(&s.support);
            let read = Favourites::load(&s.support);
            s.nav.favourites_read = read.clone().unwrap_or_default();
            s.nav.favourites = read.unwrap_or_else(|| Favourites::seeded(&roots));
            let failure = s.save_favourites();
            let former = oni_library::library::load(&s.support).files;
            let opened = remembered_opened.or_else(|| roots.current().map(str::to_owned));
            if s.nav.favourites.paths.is_empty() && opened.is_none() {
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
        if let Some(w) = self.window() {
            let n = w.global::<crate::Navigator>();
            n.set_height(height as f32);
            n.set_catch_up_shown(catching_up);
        }
        self.publish_navigator();
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
                let answering: Vec<String> = refused.lock().expect("refused").iter().filter(|f| std::fs::read_dir(f).is_ok()).cloned().collect();
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
            let before = std::mem::replace(&mut s.nav.model.volumes, now.clone());
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
            s.nav.tried.retain(|f| !on(&arrived, f));
            if !answering.is_empty() {
                s.nav.model.retry();
            }
            let opened = s.opened.clone();
            let refused = opened.as_ref().is_some_and(|o| answering.contains(o) && s.survey.unreadable.contains(o));
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
            self.survey_favourites();
        }
        self.publish_navigator();
        if moved {
            self.retry_unsaved(false);
        }
    }
}
