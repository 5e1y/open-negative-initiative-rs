//! The previews kept, a few frames' decodes the walk and the export reuse.

use super::*;

/// The last previews decoded, so reopening a frame or opening one the thumbnails just read skips
/// the decode; four previews are about 170 MB of GPU memory.
pub struct Previews {
    kept: VecDeque<(String, Arc<Negative>)>,
}

pub(crate) const KEPT_PREVIEWS: usize = 4;

impl Default for Previews {
    fn default() -> Self {
        Previews { kept: VecDeque::with_capacity(KEPT_PREVIEWS) }
    }
}

impl Previews {
    /// Forgets a frame's preview, which a new reading of its file makes stale.
    pub fn remove(&mut self, path: &str) {
        self.kept.retain(|(p, _)| p != path);
    }

    /// Forgets every preview, which another way of reading the files makes stale.
    pub fn clear(&mut self) {
        self.kept.clear();
    }

    pub fn get(&mut self, path: &str) -> Option<Arc<Negative>> {
        let i = self.kept.iter().position(|(p, _)| p == path)?;
        let entry = self.kept.remove(i).expect("found above");
        let negative = entry.1.clone();
        self.kept.push_back(entry);
        Some(negative)
    }

    pub fn insert(&mut self, path: String, negative: Arc<Negative>) {
        self.kept.retain(|(p, _)| *p != path);
        if self.kept.len() >= KEPT_PREVIEWS {
            self.kept.pop_front();
        }
        self.kept.push_back((path, negative));
    }
}
