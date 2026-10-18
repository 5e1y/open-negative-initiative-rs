//! The textures a previous render left, reused while their node and region still hold.

use super::*;

impl Cache {
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.iter().flatten().count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Node `index`'s result when it is current and covers `need`.
    pub(super) fn hit(&self, index: usize, key: u64, need: Rect) -> Option<(FloatTexture, Rect)> {
        match self.entries.get(index) {
            Some(Some(e)) if e.key == key && e.rect.contains(&need) => Some((e.texture.clone(), e.rect)),
            _ => None,
        }
    }

    /// Stores node `index`'s result, handing back the one it replaces.
    pub(super) fn store(&mut self, index: usize, key: u64, rect: Rect, texture: FloatTexture) -> Option<FloatTexture> {
        if self.entries.len() <= index {
            self.entries.resize_with(index + 1, || None);
        }
        self.entries[index].replace(Entry { key, rect, texture }).map(|e| e.texture)
    }

    /// Drops results past the graph's last node, left by a larger one.
    pub(super) fn truncate(&mut self, nodes: usize) -> Vec<FloatTexture> {
        if self.entries.len() <= nodes {
            return Vec::new();
        }
        self.entries.drain(nodes..).flatten().map(|e| e.texture).collect()
    }
}
