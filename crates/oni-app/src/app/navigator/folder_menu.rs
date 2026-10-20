//! The folder menu: open, favourite, rename, new folder, make a roll or back to a folder, show in Finder.

use super::*;

impl Ctl {
    /// `NavigatorRow.contextMenu`: Open, the favourite, Rename…, the roll, New Folder Inside, Show in Finder.
    pub(super) fn navigator_row_menu(&self, i: usize) -> bool {
        let Some(w) = self.window() else { return false };
        let lines = {
            let mut s = self.state.borrow_mut();
            let Some(path) = s.nav.rows.get(i).cloned() else { return false };
            let present = s.present(&path);
            let favourite = s.nav.favourites.contains(&path);
            let roll = s.nav.rolls().contains(&path);
            let items = [
                (line("Open", present, false), Act::Open),
                (line(if favourite { "Remove from Favourites" } else { "Add to Favourites" }, true, false), Act::Favourite),
                (line("Rename…", present, true), Act::Rename),
                (line(if roll { "Back to a Folder" } else { "Make a Roll" }, present, false), Act::Roll(!roll)),
                (line("New Folder Inside", present, false), Act::NewFolder),
                (line("Show in Finder", true, true), Act::Reveal),
            ];
            s.nav.menu = items.iter().map(|(_, a)| *a).collect();
            s.nav.menu_path = Some(path);
            items.into_iter().map(|(l, _)| l).collect::<Vec<_>>()
        };
        w.global::<crate::Navigator>().set_menu_lines(ModelRc::new(VecModel::from(lines)));
        true
    }

    pub(super) fn navigator_menu_chosen(self: &Rc<Self>, i: usize) {
        let (act, path) = {
            let s = self.state.borrow();
            let (Some(act), Some(path)) = (s.nav.menu.get(i).copied(), s.nav.menu_path.clone()) else { return };
            (act, path)
        };
        match act {
            Act::Open => {
                self.state.borrow_mut().nav.model.selected = Some(path.clone());
                self.open_folder(Path::new(&path));
            }
            Act::Favourite => self.toggle_favourite(&path),
            Act::Rename => {
                self.state.borrow_mut().nav.model.renaming = Some(path);
                self.publish_navigator();
            }
            Act::Roll(make) => self.make_roll(&path, make),
            Act::NewFolder => self.new_folder(&path, true),
            Act::Reveal => crate::system::reveal(Path::new(&path)),
        }
    }

    /// `toggleFavourite`: kept in `favourites.json`, the read of a folder no longer wanted stopped,
    /// a new one read in turn.
    pub(in crate::app) fn toggle_favourite(&self, path: &str) {
        let failure = {
            let mut s = self.state.borrow_mut();
            s.nav.favourites.toggle(path);
            let failure = s.save_favourites();
            let wanted = s.wanted_folders();
            s.nav.surveys.yield_keeping(&wanted);
            failure
        };
        if let Some(message) = failure {
            self.say(&message);
        }
        self.start_pass();
        self.publish_navigator();
    }

    /// `FolderNavigator.commit`: an empty or unchanged name puts the row back as it was.
    pub(super) fn rename_committed(self: &Rc<Self>, i: usize, draft: &str) {
        let path = {
            let mut s = self.state.borrow_mut();
            let Some(path) = s.nav.rows.get(i).cloned() else { return };
            if s.nav.model.renaming.as_deref() != Some(path.as_str()) {
                return;
            }
            s.nav.model.renaming = None;
            path
        };
        let name = trim_spaces(draft);
        if name.is_empty() || name == paths::name(&path) {
            return self.publish_navigator();
        }
        self.rename_folder(&path, name);
    }

    /// `renameFolder`: the folder renamed on the disk; the navigator, the favourites, the opened
    /// folder, a roll's name and the index rows follow it, then the folder is read again.
    pub(in crate::app) fn rename_folder(self: &Rc<Self>, path: &str, name: &str) {
        let trimmed = trim_spaces(name);
        // A separator would move the folder rather than name it: `\` is one on Windows.
        let separator = trimmed.contains('/') || (cfg!(windows) && trimmed.contains('\\'));
        if trimmed.is_empty() || separator || trimmed == paths::name(path) {
            return self.publish_navigator();
        }
        // The open frame's settings reach its sidecar before the folder holding it moves.
        let unsaved = self.state.borrow_mut().store.flush();
        if let Some(e) = unsaved {
            self.went_wrong(&format!("The folder was not renamed, as the open photo's settings could not be saved: {e}"));
            return self.publish_navigator();
        }
        let target = paths::join(&paths::parent(path), trimmed);
        // `moveItem` refuses an existing target; `rename(2)` would replace an empty folder.
        let moved = if Path::new(&target).exists() && !same_item(path, &target) {
            Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, format!("an item named “{trimmed}” already exists")))
        } else {
            std::fs::rename(path, &target)
        };
        if let Err(e) = moved {
            self.went_wrong(&format!("The folder could not be renamed: {e}"));
            return self.publish_navigator();
        }
        let failure = {
            let mut s = self.state.borrow_mut();
            s.nav.model.follow(path, &target);
            s.nav.favourites.follow(path, &target);
            let failure = s.save_favourites();
            if let Some(opened) = s.opened.clone().filter(|o| holds(path, o)) {
                let moved = format!("{target}{}", &opened[path.len()..]);
                let _ = s.preferences.set(crate::preferences::OPENED_FOLDER, Value::String(moved.clone()));
                s.opened = Some(moved);
            }
            failure
        };
        if let Some(message) = failure {
            self.say(&message);
        }
        if let Some(mut film) = folders::roll(&target) {
            film.name = trimmed.to_owned();
            let _ = folders::set_roll(Some(&film), &target);
        }
        // A read in flight under the old names is superseded, never laid over the moved rows.
        self.state.borrow().generation.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.followed(&[Move { from: path.to_owned(), to: target }]);
        self.start_pass();
        self.publish_navigator();
    }

    /// `newFolder(in:)`: "New Folder", numbered as the Finder numbers one taken; from the navigator
    /// `made` then picks its row and names it at once, from the archive tree nothing does.
    pub(in crate::app) fn new_folder(self: &Rc<Self>, folder: &str, named: bool) {
        let mut name = "New Folder".to_string();
        let mut rank = 2;
        while Path::new(&paths::join(folder, &name)).exists() {
            name = format!("New Folder {rank}");
            rank += 1;
        }
        let path = paths::join(folder, &name);
        if std::fs::create_dir(&path).is_err() {
            return;
        }
        let made = paths::canonical(Path::new(&path));
        let inside = {
            let mut s = self.state.borrow_mut();
            let inside = s.opened.as_deref().is_some_and(|o| holds(o, &made));
            if inside {
                s.survey.folders.push(made.clone());
                if let Some(archive) = s.archive.as_mut() {
                    archive.expanded.insert(NodeId::Folder(folder.to_owned()));
                }
            }
            if named {
                s.nav.model.made(&made);
            }
            inside
        };
        if inside {
            self.lay_archive();
        }
        self.publish_navigator();
    }

    /// `makeRoll` and `unmakeRoll`: the folder's roll marker written or taken away.
    pub(in crate::app) fn make_roll(self: &Rc<Self>, folder: &str, make: bool) {
        if make {
            let film = oni_settings::Film::new(&paths::name(folder));
            if folders::set_roll(Some(&film), folder).is_err() {
                return self.went_wrong("The folder could not be made a roll: it cannot be written to.");
            }
        } else if folders::set_roll(None, folder).is_err() {
            return;
        }
        self.state.borrow_mut().nav.roll_edits.insert(folder.to_owned(), make);
        self.lay_archive();
        self.publish_navigator();
    }

    /// `reportFailure(as: "Something Went Wrong")`.
    pub(super) fn went_wrong(&self, message: &str) {
        let Some(w) = self.window() else { return };
        let a = w.global::<Advice>();
        a.set_title("Something Went Wrong".into());
        a.set_message(message.into());
        a.set_shown(true);
    }

    /// `add(_:)`'s `folderNavigation.reveal`: a folder opened from elsewhere is picked in the column.
    pub(in crate::app) fn navigator_reveal(&self, folder: &Path) {
        {
            let mut s = self.state.borrow_mut();
            let computer = s.nav.computer.clone();
            s.nav.model.reveal(&paths::canonical(folder), &computer);
        }
        self.publish_navigator();
    }
}
