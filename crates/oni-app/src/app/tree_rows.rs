//! A tree's rows as Slint reads them, and its scroll: every row, which Rust and the checks address by
//! index, and the rows near the column's view, the only ones Slint makes, so a long tree costs what it shows.

use super::tree_column::Which;
use super::{Ctl, State};
use crate::sidebar::Frame;
use crate::thumbnails;
use crate::{Library, TreeRow};
use slint::{ComponentHandle, Image, Model, ModelNotify, ModelRc, ModelTracker};
use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::rc::Rc;

struct Inner {
    rows: RefCell<Vec<TreeRow>>,
    /// The rows near the view, as indices into `rows`.
    near: Cell<(usize, usize)>,
    all_notify: ModelNotify,
    near_notify: ModelNotify,
}

/// Every row of a tree, each carrying its own index.
#[derive(Clone)]
pub(super) struct TreeRows(Rc<Inner>);

/// The rows near the view, a window over the same rows.
struct NearRows(Rc<Inner>);

/// Whether two rows draw alike: pictures by whether they are held, never pixel by pixel, as a
/// redrawn picture reaches its row apart (`fill_tree_thumbnails`).
pub(super) fn same(a: &TreeRow, b: &TreeRow) -> bool {
    let bare = |r: &TreeRow| TreeRow { picture: Image::default(), ..r.clone() };
    bare(a) == bare(b) && (a.picture.size().width > 0) == (b.picture.size().width > 0)
}

impl TreeRows {
    pub(super) fn new() -> TreeRows {
        TreeRows(Rc::new(Inner { rows: RefCell::default(), near: Cell::new((0, 0)), all_notify: ModelNotify::default(), near_notify: ModelNotify::default() }))
    }

    /// The model of every row, and of the rows near the view, for the tree's two properties.
    pub(super) fn models(&self) -> (ModelRc<TreeRow>, ModelRc<TreeRow>) {
        (ModelRc::new(self.clone()), ModelRc::new(NearRows(self.0.clone())))
    }

    /// The rows laid again: only those that changed are told when the count holds, so a click or a
    /// newly opened photo remakes no row; a count changed tells every row.
    pub(super) fn replace(&self, rows: Vec<TreeRow>) {
        let inner = &self.0;
        let count = inner.rows.borrow().len();
        if count != rows.len() {
            *inner.rows.borrow_mut() = rows;
            let (start, end) = inner.near.get();
            let len = inner.rows.borrow().len();
            inner.near.set((start.min(len), end.min(len)));
            inner.all_notify.reset();
            inner.near_notify.reset();
            return;
        }
        for (i, row) in rows.into_iter().enumerate() {
            let changed = !same(&inner.rows.borrow()[i], &row);
            if changed {
                self.set_row_data(i, row);
            }
        }
    }

    /// The rows near the view become `range`: rows leaving it are dropped and rows entering it made,
    /// the rest kept as they are.
    pub(super) fn show(&self, range: Range<usize>) {
        let inner = &self.0;
        let len = inner.rows.borrow().len();
        let (c, d) = (range.start.min(len), range.end.min(len).max(range.start.min(len)));
        let (a, b) = inner.near.get();
        if (a, b) == (c, d) {
            return;
        }
        if c >= b || d <= a || a == b {
            inner.near.set((c, d));
            return inner.near_notify.reset();
        }
        let (mut start, mut end) = (a, b);
        if c > start {
            inner.near.set((c, end));
            inner.near_notify.row_removed(0, c - start);
            start = c;
        }
        if d < end {
            inner.near.set((start, d));
            inner.near_notify.row_removed(d - start, end - d);
            end = d;
        }
        if c < start {
            inner.near.set((c, end));
            inner.near_notify.row_added(0, start - c);
            start = c;
        }
        if d > end {
            inner.near.set((start, d));
            inner.near_notify.row_added(end - start, d - end);
        }
    }

    /// Whether a row is marked selected, read without copying the row.
    pub(super) fn selected(&self, i: usize) -> Option<bool> {
        self.0.rows.borrow().get(i).map(|r| r.selected)
    }
}

fn indexed(rows: &[TreeRow], i: usize) -> Option<TreeRow> {
    rows.get(i).map(|r| TreeRow { index: i as i32, ..r.clone() })
}

impl Model for TreeRows {
    type Data = TreeRow;

    fn row_count(&self) -> usize {
        self.0.rows.borrow().len()
    }

    fn row_data(&self, row: usize) -> Option<TreeRow> {
        indexed(&self.0.rows.borrow(), row)
    }

    fn set_row_data(&self, row: usize, data: TreeRow) {
        let inner = &self.0;
        {
            let mut rows = inner.rows.borrow_mut();
            let Some(slot) = rows.get_mut(row) else { return };
            *slot = data;
        }
        inner.all_notify.row_changed(row);
        let (a, b) = inner.near.get();
        if (a..b).contains(&row) {
            inner.near_notify.row_changed(row - a);
        }
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.0.all_notify
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Model for NearRows {
    type Data = TreeRow;

    fn row_count(&self) -> usize {
        let (a, b) = self.0.near.get();
        b - a
    }

    fn row_data(&self, row: usize) -> Option<TreeRow> {
        let (a, b) = self.0.near.get();
        (a + row < b).then(|| indexed(&self.0.rows.borrow(), a + row)).flatten()
    }

    fn model_tracker(&self) -> &dyn ModelTracker {
        &self.0.near_notify
    }
}

/// The tree's delivery to Slint and its scroll, both ways.
impl Ctl {
    /// The tree's rows as Slint holds them, made and given to both of its properties the first time.
    fn column_rows(&self, which: Which) -> Option<TreeRows> {
        let w = self.window()?;
        let lib = w.global::<Library>();
        let model = match which {
            Which::Archive => lib.get_rows(),
            Which::Projects => lib.get_project_rows(),
        };
        if let Some(rows) = model.as_any().downcast_ref::<TreeRows>() {
            return Some(rows.clone());
        }
        let rows = TreeRows::new();
        let (all, near) = rows.models();
        match which {
            Which::Archive => {
                lib.set_rows(all);
                lib.set_near_rows(near);
            }
            Which::Projects => {
                lib.set_project_rows(all);
                lib.set_project_near_rows(near);
            }
        }
        Some(rows)
    }

    /// Rows laid by `lay_column` reach Slint: only the rows that changed are told, and only the rows
    /// near the view are made.
    pub(super) fn deliver_column(&self, which: Which, rows: Vec<TreeRow>) {
        let Some(model) = self.column_rows(which) else { return };
        model.replace(rows);
        self.show_near(which);
    }

    /// The rows Slint makes: those within the near-view rule's reach, none while the column has no size.
    pub(super) fn show_near(&self, which: Which) {
        let Some(model) = self.column_rows(which) else { return };
        let top = self.column_top(which);
        let range = {
            let s = self.state.borrow();
            let laid = s.laid(which);
            let height = laid.size.1;
            if height <= 0.0 {
                0..0
            } else {
                // Rows run down the column, so their tops and bottoms both rise with the index.
                let start = laid.frames.partition_point(|f| f.y + f.height <= top - height);
                let end = laid.frames.partition_point(|f| f.y < top + 2.0 * height);
                start..end.max(start)
            }
        };
        model.show(range);
    }

    /// Whether a row lies within a screen of the column's view, the rows that keep their pictures.
    pub(super) fn near_view(&self, which: Which) -> impl Fn(&Frame) -> bool + use<> {
        let top = self.column_top(which);
        // A column not laid out yet has no view: every row is near until it has one.
        let height = self.state.borrow().laid(which).size.1;
        move |f: &Frame| height <= 0.0 || (f.y + f.height > top - height && f.y < top + 2.0 * height)
    }

    /// How far the tree has scrolled, in points from its top.
    pub(super) fn column_top(&self, which: Which) -> f32 {
        let Some(w) = self.window() else { return 0.0 };
        let lib = w.global::<Library>();
        -match which {
            Which::Archive => lib.get_tree_viewport_y(),
            Which::Projects => lib.get_project_viewport_y(),
        }
    }

    /// The column asked to stand at `top`; its scroller says so back, which fills the rows come near.
    pub(super) fn scroll_column(&self, which: Which, top: f32) {
        let Some(w) = self.window() else { return };
        let lib = w.global::<Library>();
        match which {
            Which::Archive => {
                lib.set_tree_viewport_y(-top);
                lib.set_tree_scroll_asks(lib.get_tree_scroll_asks().wrapping_add(1));
            }
            Which::Projects => {
                lib.set_project_viewport_y(-top);
                lib.set_project_scroll_asks(lib.get_project_scroll_asks().wrapping_add(1));
            }
        }
    }

    /// The column's scroll moved, by the wheel, the knob or an ask: the row pinned and the pictures
    /// near the view follow. Slint runs the change handler once a frame, however many events moved it.
    pub(super) fn column_scrolled(&self, which: Which, y: f32) {
        let Some(w) = self.window() else { return };
        let lib = w.global::<Library>();
        match which {
            Which::Archive => lib.set_tree_viewport_y(y),
            Which::Projects => lib.set_project_viewport_y(y),
        }
        self.pin_column(which);
        self.fill_tree_thumbnails(None);
    }

    /// Whether a photo's thumbnail file may be read: one is known, not in memory, not asked yet.
    pub(super) fn askable(s: &State, fingerprint: &str) -> bool {
        !fingerprint.starts_with(thumbnails::PENDING) && !s.thumbs.contains(fingerprint) && !s.asked.contains(fingerprint)
    }

    pub(super) fn ask_rows(s: &mut State, asking: Vec<String>) {
        for fingerprint in asking {
            if s.asked.insert(fingerprint.clone()) {
                let _ = s.disk.send(fingerprint);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(n: usize) -> Vec<TreeRow> {
        (0..n).map(|i| TreeRow { y: i as f32 * 10.0, title: format!("R{i}").into(), ..TreeRow::default() }).collect()
    }

    fn near_titles(m: &ModelRc<TreeRow>) -> Vec<String> {
        m.iter().map(|r| format!("{}@{}", r.title, r.index)).collect()
    }

    #[test]
    fn the_rows_near_the_view_follow_it_and_carry_their_index() {
        let t = TreeRows::new();
        let (all, near) = t.models();
        t.replace(rows(10));
        assert_eq!(all.row_count(), 10);
        assert_eq!(near.row_count(), 0, "no view, no row made");
        t.show(2..5);
        assert_eq!(near_titles(&near), ["R2@2", "R3@3", "R4@4"]);
        t.show(4..7);
        assert_eq!(near_titles(&near), ["R4@4", "R5@5", "R6@6"]);
        t.show(1..3);
        assert_eq!(near_titles(&near), ["R1@1", "R2@2"]);
        t.show(0..10);
        assert_eq!(near.row_count(), 10);
        t.show(3..4);
        assert_eq!(near_titles(&near), ["R3@3"]);
        // Twin: a range past the rows is held within them.
        t.show(8..40);
        assert_eq!(near_titles(&near), ["R8@8", "R9@9"]);
    }

    #[test]
    fn a_row_changed_reaches_the_near_rows_and_a_relay_tells_only_the_rows_that_changed() {
        let t = TreeRows::new();
        let (all, near) = t.models();
        t.replace(rows(6));
        t.show(1..4);
        all.set_row_data(2, TreeRow { title: "changed".into(), ..all.row_data(2).unwrap() });
        assert_eq!(near.row_data(1).unwrap().title, "changed");
        // Twin: a row outside the view changes the whole list alone.
        all.set_row_data(5, TreeRow { title: "far".into(), ..all.row_data(5).unwrap() });
        assert_eq!(near_titles(&near), ["R1@1", "changed@2", "R3@3"]);
        assert_eq!(all.row_data(5).unwrap().title, "far");
        // A relay of the same count keeps the view; a shorter one holds it within the rows.
        let mut again = rows(6);
        again[3].selected = true;
        t.replace(again);
        assert!(near.row_data(2).unwrap().selected && !near.row_data(1).unwrap().selected);
        t.replace(rows(2));
        assert_eq!(near_titles(&near), ["R1@1"]);
    }
}
