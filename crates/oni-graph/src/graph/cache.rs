//! The textures a previous render left, reused while their node and region still hold.

use super::*;

impl Cache {
    /// A cache of the results the next notch starts from: the unchanged nodes a changed node reads,
    /// one or two textures where every stage's would weigh a gigabyte at the canvas's size.
    pub fn frontier() -> Cache {
        Cache { frontier: true, ..Cache::default() }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.seen.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The node keyed `key`'s result when it covers `need`.
    pub(super) fn hit(&self, key: u64, need: Rect) -> Option<(FloatTexture, Rect)> {
        self.entries.get(&key).filter(|e| e.rect.contains(&need)).map(|e| (e.texture.clone(), e.rect))
    }

    /// Before a render of `nodes`: which results it keeps, measured against the last render's nodes.
    pub(super) fn begin(&mut self, nodes: &[Flat]) {
        self.keep = self.frontier.then(|| {
            let changed = |i: usize| !self.seen.contains(&nodes[i].key);
            let mut keep = HashSet::new();
            for (i, node) in nodes.iter().enumerate() {
                let Kind::Stage { input, aux, .. } = &node.kind else { continue };
                if !changed(i) {
                    continue;
                }
                for &j in std::iter::once(input).chain(aux) {
                    if !changed(j) && matches!(nodes[j].kind, Kind::Stage { .. }) {
                        keep.insert(nodes[j].key);
                    }
                }
            }
            keep
        });
        self.seen = nodes.iter().map(|n| n.key).collect();
    }

    /// Stores node `key`'s result when this render keeps it, handing back the one it replaces.
    pub(super) fn store(&mut self, key: u64, rect: Rect, texture: &FloatTexture) -> Option<FloatTexture> {
        if self.keep.as_ref().is_some_and(|k| !k.contains(&key)) {
            return None;
        }
        self.entries.insert(key, Entry { rect, texture: texture.clone() }).map(|e| e.texture)
    }

    /// After a render: the results it no longer keeps, for the pool.
    pub(super) fn settle(&mut self) -> Vec<FloatTexture> {
        let (seen, keep) = (&self.seen, &self.keep);
        let mut gone = Vec::new();
        self.entries.retain(|key, e| {
            let kept = keep.as_ref().map_or_else(|| seen.contains(key), |k| k.contains(key));
            if !kept {
                gone.push(e.texture.clone());
            }
            kept
        });
        gone
    }
}
