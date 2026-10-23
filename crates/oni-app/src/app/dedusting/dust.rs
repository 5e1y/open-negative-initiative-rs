use std::collections::{HashMap, HashSet};

/// De-dust's state: each correction's number and patch source on the open frame, which another
/// frame starts again, and the frames healing and the patches made or failed, kept by id.
#[derive(Default)]
pub(in crate::app) struct Dust {
    /// The number each correction's marker shows, never its row, which a removal shifts.
    pub numbers: HashMap<oni_settings::id::Id, usize>,
    pub next_number: usize,
    /// Where each correction's patch was copied from, in fractions of the rendered frame, y down.
    pub sources: HashMap<oni_settings::id::Id, (f64, f64)>,
    /// The corrections whose search found no match and laid the tile beside the stroke.
    pub fallback: HashSet<oni_settings::id::Id>,
    /// The frames whose dust patches are being generated; the corrections whose patch is on disk,
    /// and those whose generation failed, neither looked for again.
    pub healing: HashSet<String>,
    pub patched: HashSet<oni_settings::id::Id>,
    pub unhealable: HashSet<oni_settings::id::Id>,
}

impl Dust {
    /// `load(_:)`: numbering restarts per photo, another photo's sources never show, and a heal that
    /// failed (a drive asleep) is tried again.
    pub fn next_photo(&mut self) {
        let (healing, patched) = (std::mem::take(&mut self.healing), std::mem::take(&mut self.patched));
        *self = Dust { next_number: 1, healing, patched, ..Dust::default() };
    }
}
