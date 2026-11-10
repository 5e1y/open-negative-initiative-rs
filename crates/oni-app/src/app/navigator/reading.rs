//! Reading the folders off this thread: each listed row on a pool where one stuck in its disk never
//! holds the others, its roll mark one look at its marker; and following the open photo.

use super::*;

/// One folder's listing: its subfolders (`None` while the disk refuses it), its state, whether
/// its roll marker is there, the stock the marker names, and whether its photos ask for Prepare Roll.
type Listing = (Option<Vec<String>>, Presence, bool, Option<&'static str>, bool);

impl Ctl {
    /// The navigator waits for the folder opened, whose read marks its rolls.
    pub(in crate::app) fn navigator_opened(&self, _folder: &str) {
        self.publish_navigator();
    }

    /// The opened folder read whole: its rolls known, the hand's marks inside it dropped.
    pub(in crate::app) fn navigator_read(self: &Rc<Self>) {
        {
            let mut s = self.state.borrow_mut();
            let rolls: HashSet<String> = s.survey.rolls.keys().cloned().collect();
            if let Some(opened) = s.opened.clone() {
                s.nav.roll_edits.retain(|k, _| !holds(&opened, k));
            }
            s.nav.opened_rolls = rolls;
        }
        self.publish_navigator();
        self.fresh_read();
    }

    /// The folders asked, listed apart: a favourite is a bookmark, so a row reads its own folder
    /// and its marker, never a tree.
    pub(super) fn list_apart(&self, wanted: Vec<String>) {
        let computer = self.state.borrow().nav.computer.clone();
        let read = move |folder: &str| -> Listing {
            let children = walk::children(folder);
            // A folder refused is told offline or missing here, off the interface thread.
            let state = if children.is_some() { Presence::Present } else { presence::of(folder, &computer) };
            let roll = children.is_some() && {
                presence::looked(folder);
                folders::marker(folder).is_file()
            };
            let stock = roll
                .then(|| oni_settings::film::roll_folder::roll(Path::new(folder)))
                .flatten()
                .and_then(|film| oni_settings::film::film_stock(film.stock.as_deref()))
                .map(|kind| kind.id);
            let unprepared = children.is_some() && super::super::organising::roll_card::unprepared(Path::new(folder));
            (children, state, roll, stock, unprepared)
        };
        favourites::read_apart(wanted, LISTERS, favourites::STUCK, read, |landed| with_shell(move |c| c.listed_apart(landed)));
    }

    fn listed_apart(self: &Rc<Self>, landed: Vec<(String, Listing)>) {
        let (listed, marked) = {
            let mut s = self.state.borrow_mut();
            let mut found = Vec::with_capacity(landed.len());
            let mut listed = Vec::with_capacity(landed.len());
            let mut marked = false;
            for (folder, (children, state, roll, stock, unprepared)) in landed {
                if children.is_some() {
                    marked |= if unprepared { s.nav.unprepared.insert(folder.clone()) } else { s.nav.unprepared.remove(&folder) };
                    // A mark of the hand's that the disk now shows is no longer needed.
                    if s.nav.roll_edits.get(&folder) == Some(&roll) {
                        s.nav.roll_edits.remove(&folder);
                    }
                    if roll {
                        s.nav.listed_rolls.insert(folder.clone());
                    } else {
                        s.nav.listed_rolls.remove(&folder);
                    }
                    match stock {
                        Some(id) => s.nav.listed_stocks.insert(folder.clone(), id),
                        None => s.nav.listed_stocks.remove(&folder),
                    };
                }
                found.push((folder.clone(), state));
                listed.push((folder, children));
            }
            s.noted(found);
            (listed, marked)
        };
        self.listed(listed);
        if marked {
            self.publish_navigator();
        }
    }

    /// Sidecars the app wrote: their rows' folders listed again, as a roll prepared or reset
    /// changes the mark its row shows.
    pub(in crate::app) fn navigator_wrote(&self, written: &[String]) {
        {
            let mut s = self.state.borrow_mut();
            let folders: HashSet<String> = written.iter().map(|p| paths::parent(p)).filter(|f| s.nav.rows.contains(f)).collect();
            for folder in folders {
                s.nav.model.list(&folder, true);
            }
        }
        self.list_wanted();
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
            if let Some(folder) = current {
                let favourites = s.nav.favourites.paths.clone();
                s.nav.model.show(&folder, &favourites);
            }
        }
        self.publish_navigator();
        if self.state.borrow().nav.current.is_some() {
            let n = w.global::<crate::Navigator>();
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
