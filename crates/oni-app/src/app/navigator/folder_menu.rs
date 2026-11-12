//! The folder menu, one for the navigator and the tree: open, favourite, rename, new folder, roll,
//! show in Finder; out of reach, only what touches no disk under it; on a folder gone Locate and Forget.

use super::*;
use oni_library::names::Platform;

/// Show in Finder, as the Explorer is named on Windows.
pub(in crate::app) fn reveal_word() -> &'static str {
    crate::platform::reveal(Platform::THIS)
}

/// Why a folder kept its name; Windows denies a folder holding a file another app has open.
fn renamed_refused(e: &std::io::Error, platform: Platform) -> String {
    if oni_library::refusal::held(e, platform) || oni_library::refusal::denied_on_windows(e, platform) {
        return tr("The folder could not be renamed: a file in it is open in another app. Close that app, then try again.").into();
    }
    tr!("The folder could not be renamed: {error}", error = oni_library::refusal::cause(e))
}

/// `from` named `to` on the disk, as `moveItem` refuses an existing target: never over an item there,
/// even one arriving after a look, as `rename(2)` replaces an empty folder; a change of case alone renames.
fn renamed_on_disk(from: &str, to: &str) -> std::io::Result<()> {
    if same_item(from, to) {
        let rename = |from: &Path, to: &Path| std::fs::rename(from, to);
        return oni_library::roll_folder::retried(Path::new(from), Path::new(to), &rename, &std::thread::sleep, Platform::THIS);
    }
    oni_library::roll_folder::rename_new(Path::new(from), Path::new(to))
}

/// "New Folder" made in `folder`, numbered as the Finder numbers one taken; the parent is in the
/// archive's spelling and the name plain, so the path joined is too.
fn made_inside(folder: &str) -> std::io::Result<String> {
    let mut name = "New Folder".to_string();
    let mut rank = 2;
    while Path::new(&paths::join(folder, &name)).exists() {
        name = format!("New Folder {rank}");
        rank += 1;
    }
    let path = paths::join(folder, &name);
    std::fs::create_dir(&path)?;
    Ok(path)
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
        let shown = if present {
            (MenuGlyph::Reveal, reveal_word(), FolderAct::Reveal)
        } else {
            (MenuGlyph::Parent, tr("Show the Parent Folder"), FolderAct::RevealParent)
        };
        let mut items = vec![
            (line(MenuGlyph::Open, tr("Open"), true, false), FolderAct::Open),
            (line(MenuGlyph::Favourite, if favourite { tr("Remove from Favourites") } else { tr("Add to Favourites") }, true, false), FolderAct::Favourite),
        ];
        if gone {
            items.push((line(MenuGlyph::Reveal, tr("Locate…"), true, false), FolderAct::Locate));
        }
        let (glyph, word) = if roll { (MenuGlyph::Folder, tr("Back to a Folder")) } else { (MenuGlyph::Roll, tr("Make a Roll")) };
        items.extend([
            (line(MenuGlyph::Edit, tr("Rename…"), present, true), FolderAct::Rename),
            (line(glyph, word, present, false), FolderAct::Roll(!roll)),
            (line(MenuGlyph::NewFolder, tr("New Folder Inside"), present, false), FolderAct::NewFolder),
            (line(shown.0, shown.1, true, true), shown.2),
        ]);
        // A disk away keeps its folders whatever: only a folder gone from a disk that answers is forgotten.
        if gone {
            items.push((line(MenuGlyph::Remove, tr("Remove from Library…"), true, true), FolderAct::Forget));
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
            let Some(path) = s.nav.rows.get(i).cloned().filter(|p| !walk::is_heading(p)) else { return false };
            let items = s.folder_lines(&path, s.row_presence(&path));
            s.nav.menu = items.iter().map(|(_, a)| *a).collect();
            s.nav.menu_path = Some((path, walk::heading_above(&s.nav.rows, i)));
            items.into_iter().map(|(l, _)| l).collect::<Vec<_>>()
        };
        w.global::<crate::Navigator>().set_menu_lines(ModelRc::new(VecModel::from(lines)));
        true
    }

    pub(super) fn navigator_menu_chosen(self: &Rc<Self>, i: usize) {
        let (act, path) = {
            let mut s = self.state.borrow_mut();
            let (Some(act), Some((path, section))) = (s.nav.menu.get(i).copied(), s.nav.menu_path.clone()) else { return };
            // Opened, renamed or filled from a row, the folder is picked or typed in that row's section.
            if matches!(act, FolderAct::Open | FolderAct::Rename | FolderAct::NewFolder) {
                s.nav.model.picked_in = section;
            }
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

    /// `toggleFavourite`: kept in the places file, a bookmark that reads nothing more; one taken
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
        // A move or a mark under way writes under the old name to its end.
        if self.busy_said(tr("Rename")) {
            return self.publish_navigator();
        }
        // A separator would move the folder rather than name it, a leading dot would hide it.
        if let Some(why) = oni_library::names::refused(trimmed, Platform::THIS) {
            self.told(tr("Rename"), &why);
            return self.publish_navigator();
        }
        // The open frame's settings and the drafts queued for its frames reach their sidecars
        // before the folder holding them moves.
        self.drain_drafts();
        let unsaved = self.state.borrow_mut().store.flush();
        if let Some(e) = unsaved {
            self.told(tr("Rename"), &tr!("The folder was not renamed, as the open photo's settings could not be saved: {error}", error = e));
            return self.publish_navigator();
        }
        let target = paths::join(&paths::parent(path), trimmed);
        let (from, to, name) = (path.to_owned(), target.clone(), trimmed.to_owned());
        let run = move || {
            renamed_on_disk(&from, &to)?;
            if let Some(mut film) = folders::roll(&to) {
                film.name = name;
                let _ = folders::set_roll(Some(&film), &to);
            }
            Ok(())
        };
        let old = path.to_owned();
        self.on_disk(tr("Rename"), run, move |c, renamed: std::io::Result<()>| match renamed {
            Ok(()) => c.follow_folder(&old, &target, &[]),
            Err(e) => {
                c.told(tr("Rename"), &renamed_refused(&e, Platform::THIS));
                c.publish_navigator();
            }
        });
        self.publish_navigator();
    }

    /// `newFolder(in:)`: the folder made off this thread, its name typed once it lands, in the column
    /// it was asked from: the navigator's row, or the tree's.
    pub(in crate::app) fn new_folder(self: &Rc<Self>, folder: &str, tree: bool) {
        if self.busy_said(tr("New Folder")) {
            return;
        }
        let (inside, folder) = (folder.to_owned(), folder.to_owned());
        self.on_disk(tr("New Folder"), move || made_inside(&inside), move |c, made| c.new_folder_landed(&folder, made, tree));
    }

    fn new_folder_landed(self: &Rc<Self>, folder: &str, made: std::io::Result<String>, tree: bool) {
        let made = match made {
            Ok(made) => made,
            Err(e) => {
                let cause = oni_library::refusal::cause(&e);
                return self.told(tr("New Folder"), &tr!("The folder could not be made inside “{folder}”: {cause}.", folder = paths::name(folder), cause = cause));
            }
        };
        let inside = {
            let mut s = self.state.borrow_mut();
            let shown = s.folder.as_mut().filter(|f| holds(&f.path, &made));
            let inside = shown.is_some();
            if let Some(shown) = shown {
                std::sync::Arc::make_mut(&mut shown.survey).folders.push(made.clone());
                if let Some(archive) = s.archive.as_mut() {
                    archive.unfold(NodeId::Folder(folder.to_owned()));
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
        let title = if make { tr("Make a Roll") } else { tr("Back to a Folder") };
        if self.busy_said(title) {
            return;
        }
        let film = make.then(|| oni_settings::Film::new(&paths::name(folder)));
        let (at, marker, folder) = (folder.to_owned(), film.clone(), folder.to_owned());
        self.on_disk(title, move || folders::set_roll(marker.as_ref(), &at), move |c, set| c.roll_made(title, &folder, film, set));
    }

    fn roll_made(self: &Rc<Self>, title: &str, folder: &str, film: Option<oni_settings::Film>, set: std::io::Result<()>) {
        if let Err(e) = set {
            let cause = oni_library::refusal::cause(&e);
            let said = match film {
                Some(_) => tr!("The folder could not be made a roll: {cause}.", cause = cause),
                None => tr!("The roll could not be made a folder again: {cause}.", cause = cause),
            };
            return self.told(title, &said);
        }
        let make = film.is_some();
        self.roll_written(folder, film);
        self.state.borrow_mut().nav.roll_edits.insert(folder.to_owned(), make);
        self.lay_archive();
        self.publish_navigator();
    }

    /// `add(_:)`'s `folderNavigation.reveal`: a folder opened from elsewhere is picked in the column,
    /// spelt as `looked` made it canonical off this thread.
    pub(in crate::app) fn navigator_reveal(&self, folder: &str) {
        {
            let mut s = self.state.borrow_mut();
            let favourites = s.nav.favourites.paths();
            s.nav.model.reveal(folder, &favourites);
        }
        self.publish_navigator();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rename_never_replaces_a_folder_made_meanwhile() {
        let root = std::env::temp_dir().join(format!("oni-rename-new-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (from, to) = (root.join("Roll 3"), root.join("Portra"));
        let (a, b) = (from.to_string_lossy().into_owned(), to.to_string_lossy().into_owned());
        // Another app makes an empty folder of the name between the look and the rename.
        std::fs::create_dir_all(&from).unwrap();
        std::fs::create_dir_all(&to).unwrap();
        let refused = renamed_on_disk(&a, &b).unwrap_err();
        assert_eq!(refused.kind(), std::io::ErrorKind::AlreadyExists);
        assert!(from.is_dir() && to.is_dir(), "both folders stay");
        // Twin: a plain rename takes the empty folder's place, which the guard exists to refuse.
        if cfg!(unix) {
            std::fs::rename(&from, &to).unwrap();
            assert!(!from.exists() && to.is_dir());
        }
        let _ = std::fs::remove_dir_all(&root);
    }
}
