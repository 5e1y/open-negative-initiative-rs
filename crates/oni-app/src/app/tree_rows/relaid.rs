//! What a re-lay of a tree did to its rows: the rows come or gone, told where the old rows and the
//! new part, so every other row keeps its element in Slint and a press it holds keeps its grab.

use crate::TreeRow;

/// At `at`, `removed` rows went and `added` came, one of the two none; every other row was kept, those
/// past `at` shifted by the difference.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::app) struct Relaid {
    pub(super) at: usize,
    pub(super) removed: usize,
    pub(super) added: usize,
}

/// Whether two rows draw one node of the tree, wherever each stands and however it is marked.
fn one_node(a: &TreeRow, b: &TreeRow) -> bool {
    (a.kind, a.depth, &a.title, &a.number) == (b.kind, b.depth, &b.title, &b.number)
}

impl Relaid {
    /// The rows drawing one node from the end on are kept, so the change stands where the two part
    /// and the rows before it keep their places.
    pub(super) fn between(old: &[TreeRow], new: &[TreeRow]) -> Relaid {
        let both = old.len().min(new.len());
        let tail = old.iter().rev().zip(new.iter().rev()).take_while(|(a, b)| one_node(a, b)).count();
        Relaid { at: both - tail, removed: old.len() - both, added: new.len() - both }
    }

    /// Where a row stands after the re-lay, none for a row it removed.
    pub(in crate::app) fn kept(&self, row: usize) -> Option<usize> {
        match row {
            r if r < self.at => Some(r),
            r if r < self.at + self.removed => None,
            r => Some(r - self.removed + self.added),
        }
    }

    /// Where a row stood before the re-lay, none for a row it added.
    pub(super) fn was(&self, row: usize) -> Option<usize> {
        Relaid { removed: self.added, added: self.removed, ..*self }.kept(row)
    }

    /// An end of a range of rows carried across the re-lay; one among the rows removed closes on them.
    pub(super) fn bound(&self, end: usize) -> usize {
        if end <= self.at { end } else { self.kept(end).unwrap_or(self.at) }
    }
}

#[cfg(test)]
mod tests {
    use super::super::TreeRows;
    use super::*;
    use slint::private_unstable_api::re_exports::{ModelChangeListener, ModelChangeListenerContainer};
    use slint::{Model, ModelRc};
    use std::cell::RefCell;
    use std::pin::Pin;

    fn rows(titles: &[&str]) -> Vec<TreeRow> {
        titles.iter().enumerate().map(|(i, t)| TreeRow { y: i as f32 * 10.0, title: (*t).into(), ..TreeRow::default() }).collect()
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    enum Told {
        Changed(usize),
        Added(usize, usize),
        Removed(usize, usize),
        Reset,
    }

    /// What a model tells its listeners, in order.
    #[derive(Default)]
    struct Heard(RefCell<Vec<Told>>);

    impl Heard {
        fn told(&self) -> Vec<Told> {
            self.0.take()
        }
    }

    impl ModelChangeListener for Heard {
        fn row_changed(self: Pin<&Self>, row: usize) {
            self.0.borrow_mut().push(Told::Changed(row));
        }
        fn row_added(self: Pin<&Self>, index: usize, count: usize) {
            self.0.borrow_mut().push(Told::Added(index, count));
        }
        fn row_removed(self: Pin<&Self>, index: usize, count: usize) {
            self.0.borrow_mut().push(Told::Removed(index, count));
        }
        fn reset(self: Pin<&Self>) {
            self.0.borrow_mut().push(Told::Reset);
        }
    }

    /// The rows told come or gone, or every row told anew.
    fn moves(told: &[Told]) -> Vec<Told> {
        told.iter().copied().filter(|t| !matches!(t, Told::Changed(_))).collect()
    }

    /// A repeater's elements as Slint keeps them: each made once, told its row again when changed.
    struct Elements {
        made: usize,
        each: Vec<(usize, String)>,
    }

    impl Elements {
        fn of(m: &ModelRc<TreeRow>) -> Elements {
            let mut e = Elements { made: 0, each: Vec::new() };
            e.hear(&[Told::Reset], m);
            e
        }

        fn hear(&mut self, told: &[Told], m: &ModelRc<TreeRow>) {
            let read = |m: &ModelRc<TreeRow>, i: usize| m.row_data(i).map(|r| format!("{}@{}", r.title, r.index)).unwrap();
            for t in told {
                match *t {
                    Told::Changed(i) => self.each[i].1 = read(m, i),
                    Told::Removed(i, n) => drop(self.each.drain(i..i + n)),
                    Told::Added(i, n) => {
                        for k in i..i + n {
                            self.made += 1;
                            self.each.insert(k, (self.made, read(m, k)));
                        }
                    }
                    Told::Reset => {
                        self.each.clear();
                        self.hear(&[Told::Added(0, m.row_count())], m);
                    }
                }
            }
        }

        fn shown(&self) -> Vec<&str> {
            self.each.iter().map(|(_, s)| s.as_str()).collect()
        }

        fn element(&self, shown: &str) -> usize {
            self.each.iter().find(|(_, s)| s == shown).unwrap_or_else(|| panic!("{shown} has an element")).0
        }
    }

    #[test]
    fn a_change_stands_where_the_rows_part_and_every_other_row_is_kept() {
        let r = Relaid::between(&rows(&["P", "R", "A", "C", "E", "Q"]), &rows(&["P", "R", "A", "C", "Q"]));
        assert_eq!(r, Relaid { at: 4, removed: 1, added: 0 });
        assert_eq!((0..6).map(|i| r.kept(i)).collect::<Vec<_>>(), [Some(0), Some(1), Some(2), Some(3), None, Some(4)]);
        assert_eq!((0..5).map(|i| r.was(i)).collect::<Vec<_>>(), [Some(0), Some(1), Some(2), Some(3), Some(5)]);
        // A row marked otherwise or moved down the column draws the same node.
        let mut longer = rows(&["P", "R", "A", "B", "C", "Q"]);
        longer[5].selected = true;
        let r = Relaid::between(&rows(&["P", "R", "A", "C", "Q"]), &longer);
        assert_eq!(r, Relaid { at: 3, removed: 0, added: 1 });
        assert_eq!((r.kept(2), r.kept(3), r.was(3), r.bound(3), r.bound(4)), (Some(2), Some(4), None, 3, 5));
        // Twin: a row of another name ends the kept tail, so the change stands past it.
        let r = Relaid::between(&rows(&["P", "R", "A", "C", "Q"]), &rows(&["P", "R", "A", "B", "C", "X"]));
        assert_eq!(r, Relaid { at: 5, removed: 0, added: 1 });
        // A count held changes rows in place.
        let r = Relaid::between(&rows(&["A", "C"]), &rows(&["C", "A"]));
        assert!(r.removed + r.added == 0 && (0..2).all(|i| r.kept(i) == Some(i)));
    }

    #[test]
    fn a_relay_of_another_count_keeps_the_element_of_every_row_it_keeps() {
        let t = TreeRows::new();
        let (all, near) = t.models();
        let ears = [(); 2].map(|_| Box::pin(ModelChangeListenerContainer::new(Heard::default())));
        all.model_tracker().attach_peer(ears[0].as_ref().model_peer());
        near.model_tracker().attach_peer(ears[1].as_ref().model_peer());
        let heard = |k: usize| ears[k].told();
        t.replace(rows(&["P", "R", "A", "C", "E", "Q", "Z"]));
        t.show(1..6);
        heard(0);
        heard(1);
        let mut elements = Elements::of(&near);
        assert_eq!(elements.shown(), ["R@1", "A@2", "C@3", "E@4", "Q@5"]);
        let (q, a) = (elements.element("Q@5"), elements.element("A@2"));

        // E goes: Q moves up a row in its own element, nothing remade.
        assert_eq!(t.replace(rows(&["P", "R", "A", "C", "Q", "Z"])), Relaid { at: 4, removed: 1, added: 0 });
        assert_eq!(moves(&heard(0)), [Told::Removed(4, 1)]);
        let told = heard(1);
        assert_eq!(moves(&told), [Told::Removed(3, 1)]);
        elements.hear(&told, &near);
        assert_eq!(elements.shown(), ["R@1", "A@2", "C@3", "Q@4"]);
        assert_eq!((elements.element("Q@4"), elements.element("A@2")), (q, a), "Q and A keep their elements");

        // B comes before C: Q moves down a row, still in its element.
        t.replace(rows(&["P", "R", "A", "B", "C", "Q", "Z"]));
        assert_eq!(moves(&heard(0)), [Told::Added(3, 1)]);
        let told = heard(1);
        assert_eq!(moves(&told), [Told::Added(2, 1)]);
        elements.hear(&told, &near);
        assert_eq!(elements.shown(), ["R@1", "A@2", "B@3", "C@4", "Q@5"]);
        assert_eq!(elements.element("Q@5"), q);

        // Twin: the same re-lay told as a reset remakes every element, Q's and the press it holds.
        elements.hear(&[Told::Reset], &near);
        assert_eq!(elements.shown(), ["R@1", "A@2", "B@3", "C@4", "Q@5"]);
        assert_ne!(elements.element("Q@5"), q, "a reset makes Q's element anew");
    }
}
