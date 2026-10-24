//! The ONI Trash: laid as a grid, its rows, Restore and Restore All.

use super::*;

impl Ctl {
    /// The ONI Trash's photos, whichever folder or disk holds them, latest deletion first, as
    /// Swift's `takeRows` reads the whole index; one on a disk away tiles as offline.
    pub(in crate::app) fn read_trashed(&self) {
        let mut s = self.state.borrow_mut();
        let Some(index) = &s.index else { return };
        s.org.trashed = index.trashed_frames();
    }

    pub(in crate::app) fn publish_trash_rows(&self) {
        let Some(w) = self.window() else { return };
        let lines: Vec<TrashLine> = {
            let s = self.state.borrow();
            let count = s.org.trashed.len();
            let unfolded = count > 0 && s.org.trash_unfolded;
            let mut lines = vec![TrashLine {
                title: tr("ONI Trash").into(),
                trail: SharedString::new(),
                count: if count > 0 { count.to_string().into() } else { SharedString::new() },
                depth: 0,
                foldable: count > 0,
                unfolded,
                picked: s.org.trash_picked.as_deref() == Some(""),
                opened: s.org.trash_open,
                present: count > 0,
                restore_help: if count > 0 { tr("Restore every photo in the ONI Trash") } else { "" }.into(),
            }];
            if unfolded {
                let home = home();
                for folder in oni_library::trash::folders(&s.org.trashed) {
                    let heading = oni_library::trash::heading(&folder.path, &home);
                    let (trail, title) = match heading.rfind('/') {
                        Some(cut) => (format!("{}/", &heading[..cut]), heading[cut + 1..].to_string()),
                        None => (String::new(), heading.clone()),
                    };
                    lines.push(TrashLine {
                        title: title.into(),
                        trail: trail.into(),
                        count: folder.count.to_string().into(),
                        depth: 1,
                        foldable: false,
                        unfolded: false,
                        picked: s.org.trash_picked.as_deref() == Some(folder.path.as_str()),
                        opened: false,
                        present: s.present(&folder.path),
                        restore_help: trn("Restore this photo", "Restore these {n} photos", folder.count as u64).into(),
                    });
                }
            }
            lines
        };
        w.global::<Library>().set_trash_rows(ModelRc::new(VecModel::from(lines)));
        self.trash_synced();
    }

    pub(super) fn trash_folder(&self, i: usize) -> Option<(String, usize)> {
        let s = self.state.borrow();
        oni_library::trash::folders(&s.org.trashed).into_iter().nth(i.checked_sub(1)?).map(|f| (f.path, f.count))
    }

    /// `onClick(1)`: the trash row folds, a folder row is picked.
    pub(in crate::app) fn trash_row_clicked(self: &Rc<Self>, i: usize) {
        if self.state.borrow_mut().second_click(Column::Trash, i) {
            return;
        }
        {
            let folder = self.trash_folder(i);
            let mut s = self.state.borrow_mut();
            match folder {
                None if i == 0 => {
                    s.org.trash_picked = Some(String::new());
                    if !s.org.trashed.is_empty() {
                        s.org.trash_unfolded = !s.org.trash_unfolded;
                    }
                }
                Some((path, _)) => s.org.trash_picked = Some(path),
                None => {}
            }
        }
        self.publish_trash_rows();
    }

    /// `onClick(2)`: the second click opens the ONI Trash, the fold the first click turned kept.
    pub(in crate::app) fn trash_row_double_clicked(self: &Rc<Self>, _: usize) {
        self.open_trash();
    }

    pub(in crate::app) fn trash_row_restore(self: &Rc<Self>, i: usize) {
        if i == 0 {
            return self.ask_restore_all();
        }
        let Some((folder, _)) = self.trash_folder(i) else { return };
        let frames: Vec<String> = {
            let s = self.state.borrow();
            let mut f: Vec<String> = oni_library::trash::frames(&folder, &s.org.trashed).into_iter().collect();
            f.sort();
            f
        };
        self.restore(&frames);
    }

    pub(in crate::app) fn trash_row_menu(self: &Rc<Self>, i: usize) -> bool {
        let count = self.state.borrow().org.trashed.len();
        // A folder's line opens the whole ONI Trash, and says so.
        let open = if i == 0 { tr("Open") } else { tr("Open the ONI Trash") };
        let mut items = vec![Item { line: menu_line(open, true, false, false), act: Act::OpenTrash }];
        if i == 0 {
            if count > 0 {
                items.push(Item { line: menu_line(tr("Restore All…"), true, false, false), act: Act::RestoreAll });
            }
        } else if let Some((folder, n)) = self.trash_folder(i) {
            let frames: Vec<String> = {
                let s = self.state.borrow();
                oni_library::trash::frames(&folder, &s.org.trashed).into_iter().collect()
            };
            let word = trn("Restore Photo", "Restore {n} Photos", n as u64);
            items.push(Item { line: menu_line(&word, true, false, false), act: Act::Restore(frames) });
            // Its volume mounted, from the path alone, as Swift tells a trash folder's row.
            let present = self.state.borrow().present(&folder);
            items.push(Item { line: menu_line(crate::app::navigator::reveal_word(), present, true, false), act: Act::Reveal(vec![folder]) });
        }
        self.open_menu(items, None)
    }

    /// `openTrash`: the grid lays the ONI Trash, both selections cleared, in the gallery.
    pub(in crate::app) fn open_trash(self: &Rc<Self>) {
        {
            let mut s = self.state.borrow_mut();
            s.org.trash_open = true;
            s.org.projects_surface = false;
            s.trash_picks = Picks::default();
            s.picks.selection.clear();
        }
        if let Some(w) = self.window() {
            w.global::<App>().set_gallery(true);
            w.global::<Library>().set_surface(0);
        }
        self.lay_grid();
        self.publish_bar();
    }

    pub(in crate::app) fn leave_trash(&self) {
        let mut s = self.state.borrow_mut();
        s.org.trash_open = false;
        s.trash_picks = Picks::default();
    }

    /// `restoreFromOniTrash`: the marks cleared on the trashed among `paths` off this thread, one gallery step.
    pub(in crate::app) fn restore(self: &Rc<Self>, paths: &[String]) {
        let targets: Vec<String> = {
            let s = self.state.borrow();
            let trashed: HashSet<&str> = s.org.trashed.iter().map(|e| e.path.as_str()).collect();
            paths.iter().filter(|p| trashed.contains(p.as_str())).cloned().collect()
        };
        self.mark_off_thread(tr("Restore"), targets, None);
    }

    /// `askRestoreAll`: every photo in the ONI Trash, once a key confirms.
    pub(super) fn ask_restore_all(self: &Rc<Self>) {
        let count = self.state.borrow().org.trashed.len();
        if count == 0 {
            return;
        }
        let title = trn("Restore This Photo?", "Restore {n} Photos?", count as u64);
        let key = trn("Restore Photo", "Restore {n} Photos", count as u64);
        self.ask(&title, tr("Every photo in the ONI Trash goes back to its folder, roll and project."), &key, false, Pending::RestoreAll);
    }

    pub(in crate::app) fn restore_all(self: &Rc<Self>) {
        let all: Vec<String> = self.state.borrow().org.trashed.iter().map(|e| e.path.clone()).collect();
        self.restore(&all);
    }

    /// `DSDialogRequest.confirm`: the sheet over the window until a key answers it; a destructive key
    /// is red and takes no Return, and quitting's alert keeps Cancel the default.
    pub(in crate::app) fn ask(&self, title: &str, message: &str, key: &str, destructive: bool, pending: Pending) {
        let quitting = matches!(pending, Pending::QuitAnyway);
        self.state.borrow_mut().pending = Some(pending);
        if let Some(w) = self.window() {
            let c = w.global::<Confirm>();
            c.set_title(title.into());
            c.set_message(message.into());
            c.set_key(key.to_uppercase().into());
            c.set_destructive(destructive);
            c.set_cancel_default(quitting);
            c.set_shown(true);
            // A field behind the sheet would keep the keys its Return and typing are meant for.
            w.invoke_refocus();
        }
    }
}
