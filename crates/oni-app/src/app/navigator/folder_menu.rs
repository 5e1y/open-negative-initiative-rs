//! The folder menu, one for the navigator and the tree: open, favourite, rename, new folder, roll,
//! show in Finder; out of reach, only what touches no disk under it; on a folder gone Locate and Forget.

use super::*;

/// Show in Finder, as the Explorer is named on Windows.
pub(in crate::app) fn reveal_word() -> &'static str {
    if cfg!(windows) { tr("Show in Explorer") } else { tr("Show in Finder") }
}

impl State {
    /// The one folder menu, the navigator's and the tree's alike: Open, the favourite, Rename…, the
    /// roll, New Folder Inside, Show in Finder; out of reach, only what writes nothing under it.
    pub(in crate::app) fn folder_lines(&self, path: &str, presence: Presence) -> Vec<(MenuLine, FolderAct)> {
        let present = presence.is_present();
        let gone = presence == Presence::Missing;
        let favourite = self.nav.favourites.contains(path);
        let roll = self.nav.rolls().contains(path);
        // Out of reach, Open shows its state, and the folder above it stands in for it in the Finder.
        let shown = if present { (reveal_word(), FolderAct::Reveal) } else { (tr("Show the Parent Folder"), FolderAct::RevealParent) };
        let mut items = vec![
            (line(tr("Open"), true, false), FolderAct::Open),
            (line(if favourite { tr("Remove from Favourites") } else { tr("Add to Favourites") }, true, false), FolderAct::Favourite),
        ];
        if gone {
            items.push((line(tr("Locate…"), true, false), FolderAct::Locate));
        }
        items.extend([
            (line(tr("Rename…"), present, true), FolderAct::Rename),
            (line(if roll { tr("Back to a Folder") } else { tr("Make a Roll") }, present, false), FolderAct::Roll(!roll)),
            (line(tr("New Folder Inside"), present, false), FolderAct::NewFolder),
            (line(shown.0, true, true), shown.1),
        ]);
        // A disk away keeps its folders whatever: only a folder gone from a disk that answers is forgotten.
        if gone {
            items.push((line(tr("Remove from Library…"), true, true), FolderAct::Forget));
        }
        items
    }
}

impl Ctl {
    /// `NavigatorRow.contextMenu`: the folder menu on the row's folder.
    pub(super) fn navigator_row_menu(&self, i: usize) -> bool {
        let Some(w) = self.window() else { return false };
        let lines = {
            let mut s = self.state.borrow_mut();
            let Some(path) = s.nav.rows.get(i).cloned() else { return false };
            let items = s.folder_lines(&path, s.row_presence(&path));
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
        self.folder_acted(act, &path, false);
    }

    /// A line of the folder menu; renaming and a new folder's name are typed in the column it was
    /// asked from, the tree's when `tree`.
    pub(in crate::app) fn folder_acted(self: &Rc<Self>, act: FolderAct, path: &str, tree: bool) {
        match act {
            FolderAct::Open => {
                self.state.borrow_mut().nav.model.selected = Some(path.to_owned());
                self.open_asked(path);
            }
            FolderAct::Favourite => self.toggle_favourite(path),
            FolderAct::Rename if tree => self.rename_in_tree(path.to_owned()),
            FolderAct::Rename => {
                self.state.borrow_mut().nav.model.renaming = Some(path.to_owned());
                self.publish_navigator();
            }
            FolderAct::Roll(make) => self.make_roll(path, make),
            FolderAct::NewFolder => self.new_folder(path, tree),
            FolderAct::Reveal => self.reveal_folder(path),
            FolderAct::RevealParent => self.reveal_folder(&paths::parent(path)),
            FolderAct::Locate => self.locate(path),
            FolderAct::Forget => self.ask_forget(path),
        }
    }

    /// `toggleFavourite`: kept in `favourites.json`, a bookmark that reads nothing more; one taken
    /// away says so, with Undo, as its row may vanish.
    pub(in crate::app) fn toggle_favourite(&self, path: &str) {
        let (failure, removed) = {
            let mut s = self.state.borrow_mut();
            let removed = s.nav.favourites.contains(path);
            s.nav.favourites.toggle(path);
            let failure = s.save_favourites();
            (failure, removed)
        };
        if let Some(message) = failure {
            self.told(tr("Something Went Wrong"), &message);
        } else if removed {
            let name = self.state.borrow().nav.computer.name(path);
            self.told_undo(tr("Removed from Favourites"), &tr!("“{name}” is no longer a favourite.", name = name), path);
        }
        self.publish_navigator();
    }

    /// `Add`: the folder starred, never taken away when it is one already.
    pub(in crate::app) fn star(&self, path: &str) {
        if !self.state.borrow().nav.favourites.contains(path) {
            self.toggle_favourite(path);
        }
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
        if trimmed.is_empty() || trimmed == paths::name(path) {
            return self.publish_navigator();
        }
        // A separator would move the folder rather than name it, a leading dot would hide it.
        if let Some(why) = oni_library::file_names::refused(trimmed, cfg!(windows)) {
            self.told(tr("Rename"), &why);
            return self.publish_navigator();
        }
        // The open frame's settings reach its sidecar before the folder holding it moves.
        let unsaved = self.state.borrow_mut().store.flush();
        if let Some(e) = unsaved {
            self.told(tr("Rename"), &tr!("The folder was not renamed, as the open photo's settings could not be saved: {error}", error = e));
            return self.publish_navigator();
        }
        let target = paths::join(&paths::parent(path), trimmed);
        // `moveItem` refuses an existing target; `rename(2)` would replace an empty folder.
        let moved = if Path::new(&target).exists() && !same_item(path, &target) {
            Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, tr!("an item named “{name}” already exists", name = trimmed)))
        } else {
            std::fs::rename(path, &target)
        };
        if let Err(e) = moved {
            self.told(tr("Rename"), &tr!("The folder could not be renamed: {error}", error = oni_library::refusal::cause(&e)));
            return self.publish_navigator();
        }
        if let Some(mut film) = folders::roll(&target) {
            film.name = trimmed.to_owned();
            let _ = folders::set_roll(Some(&film), &target);
        }
        self.follow_folder(path, &target);
    }

    /// `newFolder(in:)`: "New Folder", numbered as the Finder numbers one taken, its name typed at
    /// once in the column it was asked from: the navigator's row, or the tree's once it lands there.
    pub(in crate::app) fn new_folder(self: &Rc<Self>, folder: &str, tree: bool) {
        let mut name = "New Folder".to_string();
        let mut rank = 2;
        while Path::new(&paths::join(folder, &name)).exists() {
            name = format!("New Folder {rank}");
            rank += 1;
        }
        let path = paths::join(folder, &name);
        if let Err(e) = std::fs::create_dir(&path) {
            let cause = oni_library::refusal::cause(&e);
            return self.told(tr("New Folder"), &tr!("The folder could not be made inside “{folder}”: {cause}.", folder = paths::name(folder), cause = cause));
        }
        let made = paths::canonical(Path::new(&path));
        let inside = {
            let mut s = self.state.borrow_mut();
            let inside = s.opened.as_deref().is_some_and(|o| holds(o, &made));
            if inside {
                std::sync::Arc::make_mut(&mut s.survey).folders.push(made.clone());
                if let Some(archive) = s.archive.as_mut() {
                    archive.expanded.insert(NodeId::Folder(folder.to_owned()));
                }
            }
            if !tree {
                s.nav.model.made(&made);
            } else if inside {
                s.rename_request = Some(made.clone());
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
            if let Err(e) = folders::set_roll(Some(&film), folder) {
                let cause = oni_library::refusal::cause(&e);
                return self.told(tr("Make a Roll"), &tr!("The folder could not be made a roll: {cause}.", cause = cause));
            }
            self.roll_written(folder, Some(film));
        } else if let Err(e) = folders::set_roll(None, folder) {
            let cause = oni_library::refusal::cause(&e);
            return self.told(tr("Back to a Folder"), &tr!("The roll could not be made a folder again: {cause}.", cause = cause));
        } else {
            self.roll_written(folder, None);
        }
        self.state.borrow_mut().nav.roll_edits.insert(folder.to_owned(), make);
        self.lay_archive();
        self.publish_navigator();
    }

    /// `add(_:)`'s `folderNavigation.reveal`: a folder opened from elsewhere is picked in the column,
    /// spelt as `looked` made it canonical off this thread.
    pub(in crate::app) fn navigator_reveal(&self, folder: &str) {
        {
            let mut s = self.state.borrow_mut();
            let computer = s.nav.computer.clone();
            s.nav.model.reveal(folder, &computer);
        }
        self.publish_navigator();
    }
}
