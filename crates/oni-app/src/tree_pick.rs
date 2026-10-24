//! `TreeEngine`'s own picks (`TreeEngine.swift:401-419`): ⌘ and ⇧ gather rows apart from the ring,
//! and a right click acts on the whole pick when it lands inside it, else on its own row.

use oni_library::archive_tree::NodeId;
use oni_library::paths;
use std::collections::HashSet;
use std::hash::Hash;

/// A tree's `selection` and `anchorRange`, which no other surface reads.
#[derive(Clone, Debug)]
pub struct TreePick<T> {
    pub selection: HashSet<T>,
    pub anchor: Option<T>,
}

impl<T> Default for TreePick<T> {
    fn default() -> Self {
        TreePick { selection: HashSet::new(), anchor: None }
    }
}

impl<T: Clone + Eq + Hash> TreePick<T> {
    /// `click`'s selection over the rows shown in `order`: true for a pick (`onPick`), false for a
    /// plain click, which empties the pick and anchors on the row (`onSelectAnchor`).
    pub fn click(&mut self, id: &T, order: &[T], command: bool, shift: bool) -> bool {
        if command {
            // The row a plain click anchored on joins the first ⌘ pick.
            if self.selection.is_empty()
                && let Some(anchor) = self.anchor.clone().filter(|a| a != id)
            {
                self.selection.insert(anchor);
            }
            if !self.selection.remove(id) {
                self.selection.insert(id.clone());
            }
            self.anchor = Some(id.clone());
            return true;
        }
        if shift && let Some(extended) = self.extended(id, order) {
            self.selection = extended;
            return true;
        }
        self.selection.clear();
        self.anchor = Some(id.clone());
        false
    }

    /// `ListPicking.extended`: the rows between the anchor and the one clicked, added to the pick;
    /// none while the anchor is not shown.
    fn extended(&self, id: &T, order: &[T]) -> Option<HashSet<T>> {
        let anchor = self.anchor.as_ref()?;
        let to = order.iter().position(|r| r == id)?;
        let from = order.iter().position(|r| r == anchor)?;
        let mut next = self.selection.clone();
        next.extend(order[from.min(to)..=from.max(to)].iter().cloned());
        Some(next)
    }

    /// The Finder rule: inside the pick, the whole pick in the order shown; else that row.
    pub fn targets(&self, id: &T, order: &[T]) -> Vec<T> {
        if !self.selection.contains(id) {
            return vec![id.clone()];
        }
        let mut out: Vec<T> = Vec::new();
        for row in order.iter().chain(self.selection.iter()) {
            if self.selection.contains(row) && !out.contains(row) {
                out.push(row.clone());
            }
        }
        out
    }

    pub fn clear(&mut self) {
        self.selection.clear();
    }
}

/// `pickInColumn`: the photos picked, and every photo surveyed under a folder picked.
pub fn picked_photos(picked: &HashSet<NodeId>, members: &[String]) -> HashSet<String> {
    let mut out = HashSet::new();
    for id in picked {
        match id {
            NodeId::Photo(path) => {
                out.insert(path.clone());
            }
            NodeId::Folder(folder) => out.extend(members.iter().filter(|m| paths::holds(folder, m)).cloned()),
        }
    }
    out
}

#[cfg(test)]
mod tests;
