//! Reading the folders off this thread: each listed row on a pool where one stuck in its disk never
//! holds the others, its roll mark one look at its marker; and following the open photo.

use super::*;

/// One folder's listing: its subfolders (`None` while the disk refuses it), its state, whether
/// its roll marker is there, and the stock the marker names.
type Listing = (Option<Vec<String>>, Presence, bool, Option<&'static str>);

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
            (children, state, roll, stock)
        };
        favourites::read_apart(wanted, LISTERS, favourites::STUCK, read, |landed| with_shell(move |c| c.listed_apart(landed)));
    }

    fn listed_apart(self: &Rc<Self>, landed: Vec<(String, Listing)>) {
        let listed = {
            let mut s = self.state.borrow_mut();
            let mut found = Vec::with_capacity(landed.len());
            let mut listed = Vec::with_capacity(landed.len());
            for (folder, (children, state, roll, stock)) in landed {
                if children.is_some() {
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
            listed
        };
        self.listed(listed);
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
