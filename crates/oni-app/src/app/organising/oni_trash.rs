//! The ONI Trash: laid as a grid, its rows, its inspector's key, Restore and Restore All.

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
        let (lines, shown): (Vec<TrashLine>, Vec<String>) = {
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
                // Empty, the ONI Trash is still there to open, never greyed as a folder away.
                present: true,
                restore_help: if count > 0 { tr("Restore every photo in the ONI Trash") } else { "" }.into(),
                help: SharedString::new(),
            }];
            let mut shown = vec![String::new()];
            if unfolded {
                let folders = oni_library::trash::folders(&s.org.trashed);
                let mut named = oni_library::trash::headings(&folders, &s.nav.computer);
                for folder in folders {
                    shown.push(folder.path.clone());
                    let (trail, title) = named.remove(&folder.path).unwrap_or_default();
                    lines.push(TrashLine {
                        title: title.into(),
                        trail: trail.into(),
                        help: folder.path.as_str().into(),
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
            (lines, shown)
        };
        self.state.borrow_mut().org.trash_lines = shown;
        w.global::<Library>().set_trash_rows(ModelRc::new(VecModel::from(lines)));
        self.trash_synced();
    }

    /// The folder row `i` shows and its photos in the ONI Trash now; none for the trash's own row.
    pub(super) fn trash_folder(&self, i: usize) -> Option<(String, usize)> {
        let s = self.state.borrow();
        shown_folder(&s.org.trash_lines, &s.org.trashed, i)
    }

    /// A click opens the ONI Trash, as a folder's opens the folder, and picks the row: the trash
    /// itself, or one of its folders; its empty grid says so.
    pub(in crate::app) fn trash_row_clicked(self: &Rc<Self>, i: usize) {
        if self.state.borrow_mut().second_click(Column::Trash, i) {
            return;
        }
        let picked = match self.trash_folder(i) {
            Some((path, _)) => path,
            None if i == 0 => String::new(),
            None => return,
        };
        self.state.borrow_mut().org.trash_picked = Some(picked);
        self.open_trash();
    }

    /// The chevron alone: the trash's folders fold or unfold, and nothing opens.
    pub(in crate::app) fn trash_row_folded(self: &Rc<Self>, i: usize) {
        {
            let mut s = self.state.borrow_mut();
            if i != 0 || s.org.trashed.is_empty() {
                return;
            }
            s.org.trash_unfolded = !s.org.trash_unfolded;
        }
        self.publish_trash_rows();
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
        self.state.borrow_mut().org.restore_told = Some((folder, frames.clone()));
        self.restore(&frames);
    }

    pub(in crate::app) fn trash_row_menu(self: &Rc<Self>, i: usize) -> bool {
        let count = self.state.borrow().org.trashed.len();
        // A folder's line opens the whole ONI Trash, and says so.
        let open = if i == 0 { tr("Open") } else { tr("Open the ONI Trash") };
        let mut items = vec![Item { line: menu_line(MenuGlyph::Open, open, true, false, false), act: Act::OpenTrash }];
        if i == 0 {
            if count > 0 {
                items.push(Item { line: menu_line(MenuGlyph::Restore, tr("Restore All…"), true, false, false), act: Act::RestoreAll });
            }
        } else if let Some((folder, n)) = self.trash_folder(i) {
            let frames: Vec<String> = {
                let s = self.state.borrow();
                oni_library::trash::frames(&folder, &s.org.trashed).into_iter().collect()
            };
            let word = trn("Restore Photo", "Restore {n} Photos", n as u64);
            items.push(Item { line: menu_line(MenuGlyph::Restore, &word, true, false, false), act: Act::Restore(frames) });
            // Its volume mounted, from the path alone, as Swift tells a trash folder's row.
            let present = self.state.borrow().present(&folder);
            items.push(Item { line: menu_line(MenuGlyph::Reveal, crate::app::navigator::reveal_word(), present, true, false), act: Act::Reveal(vec![folder]) });
        }
        self.open_menu(items, None)
    }

    /// `openTrash`: the grid lays the ONI Trash, both selections cleared, in the gallery.
    pub(in crate::app) fn open_trash(self: &Rc<Self>) {
        self.leave_editor();
        {
            let mut s = self.state.borrow_mut();
            s.org.trash_open = true;
            s.org.projects_surface = false;
            s.trash_picks = Picks::default();
            s.picks.selection.clear();
        }
        if let Some(w) = self.window() {
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

    /// The inspector's key: the photos picked go back as ⇧⌘⌫ sends them; none picked, Restore All asks.
    pub(in crate::app) fn trash_restore(self: &Rc<Self>) {
        if self.state.borrow().trash_picks.selection.is_empty() {
            self.ask_restore_all();
        } else {
            self.act(crate::Act::Restore);
        }
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
            c.set_key(key.into());
            c.set_destructive(destructive);
            c.set_cancel_default(quitting);
            c.set_shown(true);
            // A field behind the sheet would keep the keys its Return and typing are meant for.
            w.invoke_refocus();
        }
    }
}

/// The inspector's count and its key's word, which names what `trash_restore` reaches: the photos
/// picked, else every one once asked; no key on an empty trash.
pub(super) fn inspector(trashed: usize, picked: usize) -> (String, String) {
    let key = match (picked, trashed) {
        (0, 0) => String::new(),
        (0, _) => tr("Restore All…").to_string(),
        (n, _) => trn("Restore Photo", "Restore {n} Photos", n as u64),
    };
    (trn("{n} photo", "{n} photos", trashed as u64), key)
}

/// The folder `lines` shows at row `i`, while the trash still holds photos of it, and how many.
fn shown_folder(lines: &[String], trashed: &[Entry], i: usize) -> Option<(String, usize)> {
    let path = lines.get(i).filter(|p| !p.is_empty())?;
    oni_library::trash::folders(trashed).into_iter().find(|f| f.path == *path).map(|f| (f.path, f.count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_settings::Date;

    #[test]
    fn a_trash_row_names_the_folder_it_showed() {
        let entry = |path: &str, at: f64| Entry { trashed_at: Some(Date::since_reference_date(at)), ..Entry::new(path, Date::since_reference_date(0.0), path) };
        let lines = ["".to_string(), "/r/A".to_string(), "/r/B".to_string()];
        // A's photos left the trash after the rows were laid: its row acts on nothing, never on B.
        let now = [entry("/r/B/b.tif", 1.0)];
        assert_eq!(shown_folder(&lines, &now, 1), None);
        assert_eq!(shown_folder(&lines, &now, 2), Some(("/r/B".to_string(), 1)));
        // Twin: A's row read as the trash's first folder cut anew, as a row index would, names B.
        assert_eq!(oni_library::trash::folders(&now).first().map(|f| f.path.clone()), Some("/r/B".to_string()));
        assert_eq!(shown_folder(&lines, &now, 0), None, "the trash's own row is no folder");
    }

    #[test]
    fn the_inspector_key_names_the_picks_else_every_photo_and_is_gone_on_an_empty_trash() {
        assert_eq!(inspector(3, 0), ("3 photos".to_string(), "Restore All…".to_string()));
        assert_eq!(inspector(3, 1).1, "Restore Photo");
        assert_eq!(inspector(3, 2).1, "Restore 2 Photos");
        assert_eq!(inspector(0, 0), ("0 photos".to_string(), String::new()));
        // Twin: a key read off the trash's size instead of the picks names photos never picked.
        let wrong = |trashed: usize, _picked: usize| trn("Restore Photo", "Restore {n} Photos", trashed as u64);
        assert_ne!(wrong(3, 1), inspector(3, 1).1);
    }
}
