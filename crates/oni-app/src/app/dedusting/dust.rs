use oni_settings::cg::Point;
use oni_settings::frame::Correction;
use oni_settings::geometry::Placing;
use std::collections::{HashMap, HashSet};

/// De-dust's state: each correction's number and patch source on the open frame, which another
/// frame starts again, and the patches made or failed, kept by id.
#[derive(Default)]
pub(in crate::app) struct Dust {
    /// The number each correction's marker shows, never its row, which a removal shifts.
    pub numbers: HashMap<oni_settings::id::Id, usize>,
    pub next_number: usize,
    /// Where each correction's patch was copied from, in fractions of the scan, y down.
    pub sources: HashMap<oni_settings::id::Id, (f64, f64)>,
    /// The correction whose tile the hand is on: the canvas shows it alone, with its source.
    pub shown: Option<oni_settings::id::Id>,
    /// The corrections whose search found no match and laid the tile beside the stroke.
    pub fallback: HashSet<oni_settings::id::Id>,
    /// The patches on disk, and those whose generation failed, by key, neither looked for again.
    pub patched: HashSet<oni_pipeline::corrections::PatchKey>,
    pub unhealable: HashSet<oni_pipeline::corrections::PatchKey>,
    /// Auto De-dust: a reading under way, one waiting on the full resolution, the last one shown.
    pub detecting: bool,
    pub detect_asked: bool,
    pub found: Option<super::auto::Shown>,
    /// The Sensitivity slider, 0 to 1, kept across photos, and its hand.
    pub sensitivity: f32,
    pub sensitivity_start: Option<f32>,
    pub sensitivity_pressed_at: Option<std::time::Instant>,
}

impl Dust {
    /// `load(_:)`: numbering restarts per photo, another photo's sources never show, and a heal that
    /// failed (a drive asleep) is tried again.
    pub fn next_photo(&mut self) {
        let patched = std::mem::take(&mut self.patched);
        *self = Dust { next_number: 1, patched, sensitivity: self.sensitivity, ..Dust::default() };
    }
}

/// The corrections as the canvas shows them, each stroke moved by its offset in the scan's fractions,
/// then mapped from the scan onto the frame stage 0 hands on; with no map, as they are.
pub(in crate::app) fn on_frame(corrections: &[Correction], offsets: impl Fn(&Correction) -> (f64, f64), placing: Option<&Placing>) -> Vec<Correction> {
    let point = |p: &Point, o: (f64, f64)| {
        let moved = Point::new(p.x + o.0, p.y + o.1);
        placing.map_or(moved, |m| m.on_frame(moved))
    };
    corrections.iter().map(|c| Correction { stroke: c.stroke.iter().map(|p| point(p, offsets(c))).collect(), ..c.clone() }).collect()
}

impl crate::app::State {
    /// The open frame's map between its scan and the picture the canvas shows.
    pub(in crate::app) fn placing(&self) -> Option<Placing> {
        let negative = self.negative()?;
        Some(self.store.settings().geometry.placing(negative.full()))
    }
}
