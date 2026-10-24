//! What one grid's pointer and keys do to its picks, `GalleryEngine.click`, `move` and `selectAll`:
//! the selection, its anchor and the keyboard's cursor, kept by path so a relayout keeps them.

use crate::picking::{self, Click, ClickTracker, SpaceKey, Step};
use std::collections::HashSet;

mod cells;
pub use cells::*;

/// A grid as the keys read it: its photos in the order shown and its bands, each named.
pub struct Shown<'a> {
    pub order: &'a [String],
    /// Each band's key and photos, in order; an empty band is skipped by every rule.
    pub groups: &'a [(String, Vec<String>)],
    pub columns: usize,
}

impl Shown<'_> {
    fn index(&self, path: &str) -> Option<usize> {
        self.order.iter().position(|p| p == path)
    }

    fn blocks(&self) -> Vec<usize> {
        self.groups.iter().map(|(_, e)| e.len()).filter(|n| *n > 0).collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridKey {
    Left,
    Right,
    Up,
    Down,
    Return,
    /// `modified`: ⌘, ⌥, ⌃ or ⇧ held, any of which leaves Space to someone else.
    Space { repeat: bool, modified: bool },
    SelectAll,
}

/// What a key or a click asks of the window beyond the picks: a photo to open, a cell to scroll to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    None,
    Open(String),
    Reveal(usize),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Picks {
    pub selection: HashSet<String>,
    pub anchor: Option<String>,
    pub cursor: Option<String>,
    clicks: ClickTracker,
}

impl Picks {
    /// A click on a cell: ⌘ toggles and anchors, ⇧ extends from the anchor shown, a plain click
    /// selects and anchors, and the second plain click within `interval` opens.
    pub fn click(&mut self, index: usize, shown: &Shown, command: bool, shift: bool, time: f64, interval: f64) -> Outcome {
        let Some(path) = shown.order.get(index).cloned() else { return Outcome::None };
        self.cursor = Some(path.clone());
        if command {
            self.selection = picking::toggled(&path, &self.selection);
            self.anchor = Some(path);
            return Outcome::None;
        }
        if shift && let Some(extended) = picking::extended(index, self.anchor.as_deref(), shown.order, &self.selection) {
            self.selection = extended;
            return Outcome::None;
        }
        self.selection = HashSet::from([path.clone()]);
        self.anchor = Some(path.clone());
        match self.clicks.register(&path, time, interval) {
            Click::Open => Outcome::Open(path),
            Click::Select => Outcome::None,
        }
    }

    /// `onClickEmpty`: the ground clears the selection.
    pub fn clear(&mut self) {
        self.selection.clear();
    }

    /// The keys of `GalleryEngine.key(stroke:)`; `None` when the key is not the grid's. `current`
    /// is the open photo, where the arrows start when no cursor is shown.
    pub fn key(&mut self, key: GridKey, shift: bool, shown: &Shown, current: Option<&str>, focus: Option<&str>) -> Option<Outcome> {
        let step = match key {
            GridKey::Left => Step::Cells(-1),
            GridKey::Right => Step::Cells(1),
            GridKey::Up => Step::Rows(-1),
            GridKey::Down => Step::Rows(1),
            GridKey::Return => return Some(self.cursor.clone().filter(|c| shown.index(c).is_some()).map_or(Outcome::None, Outcome::Open)),
            GridKey::Space { repeat, modified } => {
                return match picking::space_key(repeat, modified) {
                    SpaceKey::Ignore => None,
                    SpaceKey::Swallow => Some(Outcome::None),
                    SpaceKey::Open => Some(
                        picking::space_target(self.cursor.as_deref(), &self.selection, shown.order)
                            .filter(|p| shown.index(p).is_some())
                            .map_or(Outcome::None, Outcome::Open),
                    ),
                };
            }
            GridKey::SelectAll => return Some(self.select_all(shown, focus)),
        };
        Some(self.step(step, shift, shown, current))
    }

    fn step(&mut self, step: Step, extend: bool, shown: &Shown, current: Option<&str>) -> Outcome {
        if shown.order.is_empty() {
            return Outcome::None;
        }
        let known = self.cursor.as_deref().and_then(|c| shown.index(c)).or_else(|| current.and_then(|c| shown.index(c)));
        let from = known.unwrap_or(0);
        let to = if known.is_none() { from } else { picking::destination(from, step, &shown.blocks(), shown.columns) };
        if to == from && known.is_some() {
            return Outcome::None;
        }
        let path = shown.order[to].clone();
        self.cursor = Some(path.clone());
        if extend && let Some(origin) = self.anchor.as_deref().and_then(|a| shown.index(a)) {
            self.selection = shown.order[origin.min(to)..=origin.max(to)].iter().cloned().collect();
        } else {
            self.selection = HashSet::from([path.clone()]);
            self.anchor = Some(path);
        }
        Outcome::Reveal(to)
    }

    fn select_all(&mut self, shown: &Shown, focus: Option<&str>) -> Outcome {
        if shown.order.is_empty() {
            return Outcome::None;
        }
        let focus = focus.map(str::to_owned);
        self.selection = picking::select_all(shown.groups, &self.selection, focus.as_ref());
        if self.cursor.as_ref().is_some_and(|c| self.selection.contains(c)) {
            return Outcome::None;
        }
        let Some(first) = shown.order.iter().position(|p| self.selection.contains(p)) else { return Outcome::None };
        self.cursor = Some(shown.order[first].clone());
        self.anchor = self.cursor.clone();
        Outcome::Reveal(first)
    }

    /// Paths gone from the grid leave the picks, as a photo moved to the ONI Trash does.
    pub fn forget(&mut self, gone: &HashSet<String>) {
        self.selection.retain(|p| !gone.contains(p));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn set(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    struct Grid {
        order: Vec<String>,
        groups: Vec<(String, Vec<String>)>,
    }

    impl Grid {
        /// Two rolls: a1…a8 then b1…b3, on six columns.
        fn two_rolls() -> Grid {
            let a: Vec<String> = (1..=8).map(|i| format!("a{i}")).collect();
            let b: Vec<String> = (1..=3).map(|i| format!("b{i}")).collect();
            Grid { order: a.iter().chain(&b).cloned().collect(), groups: vec![("A".into(), a), ("B".into(), b)] }
        }

        fn shown(&self) -> Shown<'_> {
            Shown { order: &self.order, groups: &self.groups, columns: 6 }
        }
    }

    #[test]
    fn a_plain_click_selects_and_anchors_command_toggles_and_shift_extends_from_the_anchor() {
        let g = Grid::two_rolls();
        let mut p = Picks::default();
        assert_eq!(p.click(1, &g.shown(), false, false, 0.0, 0.5), Outcome::None);
        assert_eq!((p.selection.clone(), p.anchor.as_deref()), (set(&["a2"]), Some("a2")));
        p.click(3, &g.shown(), true, false, 1.0, 0.5);
        assert_eq!((p.selection.clone(), p.anchor.as_deref()), (set(&["a2", "a4"]), Some("a4")));
        p.click(6, &g.shown(), false, true, 2.0, 0.5);
        assert_eq!(p.selection, set(&["a2", "a4", "a5", "a6", "a7"]), "⇧ adds the run from the anchor, the rest kept");
        assert_eq!(p.anchor.as_deref(), Some("a4"), "⇧ moves no anchor");
        // Twin: ⇧ with no anchor shown falls back to a plain click.
        let mut fresh = Picks::default();
        fresh.click(2, &g.shown(), false, true, 0.0, 0.5);
        assert_eq!((fresh.selection, fresh.anchor.as_deref()), (set(&["a3"]), Some("a3")));
    }

    #[test]
    fn only_the_second_plain_click_within_the_interval_opens() {
        let g = Grid::two_rolls();
        let mut p = Picks::default();
        p.click(0, &g.shown(), false, false, 10.0, 0.5);
        assert_eq!(p.click(0, &g.shown(), false, false, 10.3, 0.5), Outcome::Open("a1".into()));
        p.click(0, &g.shown(), false, false, 20.0, 0.5);
        assert_eq!(p.click(0, &g.shown(), false, false, 20.7, 0.5), Outcome::None, "too slow");
        p.click(0, &g.shown(), false, false, 30.0, 0.5);
        // A ⌘-click between the two registers nothing, so the pair still opens.
        p.click(1, &g.shown(), true, false, 30.1, 0.5);
        assert_eq!(p.click(0, &g.shown(), false, false, 30.2, 0.5), Outcome::Open("a1".into()));
    }

    #[test]
    fn arrows_move_the_cursor_and_shift_arrows_extend_from_the_anchor() {
        let g = Grid::two_rolls();
        let mut p = Picks::default();
        assert_eq!(p.key(GridKey::Right, false, &g.shown(), None, None), Some(Outcome::Reveal(0)), "no cursor: the first cell");
        assert_eq!(p.key(GridKey::Down, false, &g.shown(), None, None), Some(Outcome::Reveal(6)));
        assert_eq!(p.key(GridKey::Down, false, &g.shown(), None, None), Some(Outcome::Reveal(8)), "into roll B, never a flat jump");
        assert_eq!(p.selection, set(&["b1"]));
        assert_eq!(p.key(GridKey::Left, true, &g.shown(), None, None), Some(Outcome::Reveal(7)));
        assert_eq!(p.selection, set(&["a8", "b1"]), "⇧← runs from the anchor b1");
        assert_eq!(p.key(GridKey::Down, false, &g.shown(), None, None), Some(Outcome::Reveal(9)));
        assert_eq!(p.key(GridKey::Down, false, &g.shown(), None, None), Some(Outcome::None), "the last row stops");
        // The open photo is where the arrows start when no cursor is shown.
        let mut editing = Picks::default();
        assert_eq!(editing.key(GridKey::Right, false, &g.shown(), Some("a3"), None), Some(Outcome::Reveal(3)));
        // Twin: a cursor not shown is no start either.
        let mut stale = Picks { cursor: Some("gone".into()), ..Picks::default() };
        assert_eq!(stale.key(GridKey::Right, false, &g.shown(), None, None), Some(Outcome::Reveal(0)));
    }

    #[test]
    fn return_and_space_open_and_command_a_completes_the_rolls_touched() {
        let g = Grid::two_rolls();
        let mut p = Picks::default();
        assert_eq!(p.key(GridKey::Return, false, &g.shown(), None, None), Some(Outcome::None));
        assert_eq!(p.key(GridKey::Space { repeat: false, modified: false }, false, &g.shown(), None, None), Some(Outcome::Open("a1".into())));
        assert_eq!(p.key(GridKey::Space { repeat: true, modified: false }, false, &g.shown(), None, None), Some(Outcome::None));
        assert_eq!(p.key(GridKey::Space { repeat: false, modified: true }, false, &g.shown(), None, None), None, "⇧Space is not the grid's");
        p.click(9, &g.shown(), false, false, 0.0, 0.5);
        assert_eq!(p.key(GridKey::Return, false, &g.shown(), None, None), Some(Outcome::Open("b2".into())));
        assert_eq!(p.key(GridKey::SelectAll, false, &g.shown(), None, None), Some(Outcome::None), "the cursor stays inside");
        assert_eq!(p.selection, set(&["b1", "b2", "b3"]));
        let mut none = Picks::default();
        assert_eq!(none.key(GridKey::SelectAll, false, &g.shown(), None, Some("A")), Some(Outcome::Reveal(0)));
        assert_eq!(none.selection.len(), 8);
        assert_eq!(none.cursor.as_deref(), Some("a1"));
        // Twin: without the focus the last roll completes.
        let mut last = Picks::default();
        last.key(GridKey::SelectAll, false, &g.shown(), None, None);
        assert_eq!(last.selection, set(&["b1", "b2", "b3"]));
    }
}
