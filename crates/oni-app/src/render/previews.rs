//! The previews kept, a few frames' decodes the walk and the export reuse.

use super::*;

/// The last previews decoded, so reopening a frame or opening one the thumbnails just read skips
/// the decode; four previews are about 170 MB of GPU memory.
pub struct Previews {
    /// Each by its path and whether it was balanced as a slide, which a film change turns over.
    kept: VecDeque<(String, bool, Arc<Negative>)>,
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
        self.kept.retain(|(p, ..)| p != path);
    }

    /// Forgets every preview, which another way of reading the files makes stale.
    pub fn clear(&mut self) {
        self.kept.clear();
    }

    /// Whether a preview of the frame is kept, read either way.
    pub fn holds(&self, path: &str) -> bool {
        self.kept.iter().any(|(p, ..)| p == path)
    }

    pub fn get(&mut self, path: &str, slide: bool) -> Option<Arc<Negative>> {
        let i = self.kept.iter().position(|(p, s, _)| p == path && *s == slide)?;
        let entry = self.kept.remove(i).expect("found above");
        let negative = entry.2.clone();
        self.kept.push_back(entry);
        Some(negative)
    }

    pub fn insert(&mut self, path: String, slide: bool, negative: Arc<Negative>) {
        self.kept.retain(|(p, ..)| *p != path);
        if self.kept.len() >= KEPT_PREVIEWS {
            self.kept.pop_front();
        }
        self.kept.push_back((path, slide, negative));
    }
}
