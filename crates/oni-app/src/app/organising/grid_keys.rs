//! The grid's keys: the arrows, Return, Space and ⌘A.

use super::*;

impl Ctl {
    /// `GalleryEngine.key(stroke:)`: the arrows, Return, Space and ⌘A, while the gallery shows.
    pub(in crate::app) fn grid_key(self: &Rc<Self>, text: &str, command: bool, shift: bool, bare: bool, repeat: bool) -> bool {
        let key = match text {
            "\u{F702}" if !command => GridKey::Left,
            "\u{F703}" if !command => GridKey::Right,
            "\u{F700}" if !command => GridKey::Up,
            "\u{F701}" if !command => GridKey::Down,
            "\r" | "\n" if !command => GridKey::Return,
            " " => GridKey::Space { repeat, modified: !bare },
            "a" | "A" if command && !shift => GridKey::SelectAll,
            _ => return false,
        };
        let (outcome, trash) = {
            let mut s = self.state.borrow_mut();
            let keys = s.band_keys();
            let order = s.order.clone();
            let shown = Shown { order: &order, groups: &keys, columns: s.layout.columns };
            let current = s.open.clone();
            let focus = current.as_deref().and_then(|c| keys.iter().find(|(_, p)| p.iter().any(|x| x == c))).map(|(k, _)| k.clone());
            let trash = s.trash_shown();
            let shift_held = shift && !matches!(key, GridKey::SelectAll);
            (s.shown_picks_mut().key(key, shift_held, &shown, current.as_deref(), focus.as_deref()), trash)
        };
        let Some(outcome) = outcome else { return false };
        match outcome {
            Outcome::Open(path) if !trash => self.open_in_editor(&path),
            Outcome::Reveal(at) => {
                let mut s = self.state.borrow_mut();
                if let Some(top) = s.layout.reveal(at, s.scroll, s.grid.1) {
                    s.scroll = top;
                }
            }
            _ => {}
        }
        self.publish_tiles();
        self.publish_organising();
        self.publish_bar();
        true
    }
}
