//! The navigator from the keyboard, as the Finder's sidebar answers it: the arrows walk and fold the
//! rows, Space folds, Return renames, ⌘↓ opens and ⌘↑ climbs to the parent.

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
    /// A key while the rows hold the keyboard, which a click on one gave them; false leaves it to the grid.
    pub(in crate::app) fn navigator_key(self: &Rc<Self>, text: &str, command: bool) -> bool {
        let Some(key) = row_key(text, command).filter(|_| self.state.borrow().nav.keys) else { return false };
        let open = {
            let mut s = self.state.borrow_mut();
            let rows = s.nav.rows.clone();
            let at = s.nav.model.selected.as_ref().and_then(|p| rows.iter().position(|r| r == p));
            let picked = at.and_then(|i| rows.get(i).cloned());
            let pick = |s: &mut State, i: usize| s.nav.model.selected = rows.get(i).cloned();
            let mut open = None;
            match (key, picked) {
                (RowKey::Up, _) => pick(&mut *s, at.map_or(0, |i| i.saturating_sub(1))),
                (RowKey::Down, _) => pick(&mut *s, at.map_or(0, |i| (i + 1).min(rows.len().saturating_sub(1)))),
                (RowKey::Unfold, Some(p)) if !s.nav.model.unfolded.contains(&p) => s.nav.model.toggle(&p),
                (RowKey::Fold, Some(p)) if s.nav.model.unfolded.contains(&p) => s.nav.model.toggle(&p),
                // Folded already, ← climbs to the row holding it, as the Finder's outline does.
                (RowKey::Fold, Some(p)) => {
                    let up = paths::parent(&p);
                    if let Some(i) = rows.iter().position(|r| *r == up) {
                        pick(&mut *s, i);
                    }
                }
                (RowKey::Toggle, Some(p)) => s.nav.model.toggle(&p),
                (RowKey::Rename, Some(p)) if s.row_presence(&p).is_present() => s.nav.model.renaming = Some(p),
                (RowKey::Open, Some(p)) => open = Some(p),
                (RowKey::Parent, _) => {
                    let computer = s.nav.computer.clone();
                    s.nav.model.go_up(&computer);
                }
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

    /// The field of a row renamed gone, the window's keys come back to the rows and the grid.
    pub(super) fn keys_back(&self) {
        if let Some(w) = self.window() {
            w.invoke_refocus();
        }
    }

    /// A click on the grid takes the keyboard back for the photos.
    pub(in crate::app) fn grid_took_keys(&self) {
        self.state.borrow_mut().nav.keys = false;
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
