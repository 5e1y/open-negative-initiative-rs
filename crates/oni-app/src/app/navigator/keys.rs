//! The navigator from the keyboard, as the Finder's sidebar answers it while its rows hold it: the
//! arrows walk the folders over the sections' headings and fold them, Space folds, Return renames,
//! ⌘↓ opens and ⌘↑ climbs to the folder holding the picked one.

use super::*;

/// What a key asks of the rows, read apart from the window so a check can drive it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::app) enum RowKey {
    Up,
    Down,
    Unfold,
    Fold,
    Toggle,
    Rename,
    Open,
    Parent,
}

/// The key's meaning in the rows, none for one they leave to the grid.
pub(in crate::app) fn row_key(text: &str, command: bool) -> Option<RowKey> {
    Some(match (text, command) {
        ("\u{F700}", false) => RowKey::Up,
        ("\u{F701}", false) => RowKey::Down,
        ("\u{F703}", false) => RowKey::Unfold,
        ("\u{F702}", false) => RowKey::Fold,
        (" ", false) => RowKey::Toggle,
        ("\r" | "\n", false) => RowKey::Rename,
        ("\u{F701}", true) => RowKey::Open,
        ("\u{F700}", true) => RowKey::Parent,
        _ => return None,
    })
}

impl Ctl {
    /// A key the rows read while they hold the keyboard, which a click on a row or Tab gives them;
    /// false leaves it to the window.
    pub(in crate::app) fn navigator_key(self: &Rc<Self>, text: &str, command: bool) -> bool {
        let Some(key) = row_key(text, command) else { return false };
        let open = {
            let mut s = self.state.borrow_mut();
            let rows = s.nav.rows.clone();
            let at = s.nav.model.picked_row(&rows);
            let picked = at.and_then(|i| rows.get(i).cloned());
            let pick = |s: &mut State, i: usize| s.nav.model.pick(&rows, i);
            // The picked row's section, whose chevron the keys fold.
            let heading = at.map_or(walk::COMPUTER_ROW, |i| walk::heading_above(&rows, i));
            let folder = |i: &usize| rows.get(*i).is_some_and(|r| !walk::is_heading(r));
            let first = (0..rows.len()).find(folder);
            let mut open = None;
            match (key, picked) {
                (RowKey::Up, _) => {
                    if let Some(i) = at.map_or(first, |at| (0..at).rev().find(folder).or(Some(at))) {
                        pick(&mut s, i);
                    }
                }
                (RowKey::Down, _) => {
                    if let Some(i) = at.map_or(first, |at| (at + 1..rows.len()).find(folder).or(Some(at))) {
                        pick(&mut s, i);
                    }
                }
                (RowKey::Unfold, Some(p)) if !s.nav.model.is_unfolded(heading, &p) => s.nav.model.toggle_in(heading, &p),
                (RowKey::Fold, Some(p)) if s.nav.model.is_unfolded(heading, &p) => s.nav.model.toggle_in(heading, &p),
                // Folded already, ← climbs to the row holding it, as the Finder's outline does; ⌘↑ alike.
                (RowKey::Fold | RowKey::Parent, Some(p)) => {
                    let up = paths::parent(&p);
                    let at = at.unwrap_or_default();
                    if let Some(i) = (0..at).rev().take_while(|&j| !walk::is_heading(&rows[j])).find(|&j| rows[j] == up) {
                        pick(&mut s, i);
                    }
                }
                (RowKey::Toggle, Some(p)) => s.nav.model.toggle_in(heading, &p),
                (RowKey::Rename, Some(p)) if s.row_presence(&p).is_present() => s.nav.model.renaming = Some(p),
                (RowKey::Open, Some(p)) => open = Some(p),
                _ => {}
            }
            open
        };
        self.publish_navigator();
        if let Some(folder) = open {
            self.open_asked(&folder);
        }
        true
    }

    /// The field of a row renamed gone, the keyboard goes back to the photos.
    pub(super) fn keys_back(&self) {
        if let Some(w) = self.window() {
            w.invoke_refocus();
        }
    }

    /// The rows took the keyboard: the opened folder picked where none is, so the ring shows where the arrows start.
    pub(in crate::app) fn rows_taken(&self) {
        {
            let mut s = self.state.borrow_mut();
            if s.nav.model.selected.is_none() {
                s.nav.model.selected = s.opened().map(str::to_owned);
            }
        }
        self.publish_navigator();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rows_read_the_finders_keys() {
        assert_eq!(row_key("\r", false), Some(RowKey::Rename), "Return renames, as in the Finder");
        assert_eq!(row_key("\u{F701}", true), Some(RowKey::Open), "⌘↓ opens");
        assert_eq!((row_key("\u{F703}", false), row_key(" ", false)), (Some(RowKey::Unfold), Some(RowKey::Toggle)));
        // Twin: a key the rows do not read is left to the grid.
        assert_eq!(row_key("a", false), None);
        assert_ne!(row_key("\u{F701}", false), Some(RowKey::Open), "a plain ↓ only walks");
    }
}
