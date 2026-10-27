//! Reading the targets: each photo's line, its EXIF and its preview.

use super::*;

impl Ctl {
    /// Each line from its sidecar or the scan's head, and each frame decoded once for its size and
    /// preview, in order, off this thread; a newer pass stops them.
    pub(super) fn read_targets(self: &Rc<Self>) {
        let (targets, generation, pass) = {
            let c = self.export.borrow();
            (c.targets.clone(), c.pass.fetch_add(1, Ordering::Relaxed) + 1, c.pass.clone())
        };
        self.read_rows();
        self.render_previews(targets, generation, pass);
    }

    /// What every line opens on: the targets, their roll, and the artist and copyright last written.
    fn row_sources(&self) -> (Vec<Target>, Vec<Option<oni_settings::Film>>, String, String) {
        let c = self.export.borrow();
        let s = self.state.borrow();
        // One map for the whole batch, the first entry of a path winning as a search would.
        let films: std::collections::HashMap<String, Option<oni_settings::Film>> =
            s.archive.as_ref().map(|a| a.entries().iter().rev().map(|e| (e.path.clone(), e.film.clone())).collect()).unwrap_or_default();
        let rolls = c.targets.iter().map(|t| films.get(oni_library::paths::normal(&t.path.to_string_lossy()).as_ref()).cloned().flatten()).collect();
        (c.targets.clone(), rolls, c.artist.clone(), c.copyright.clone())
    }

    /// The lines land one by one from their own thread, so the surface shows before any scan is read.
    fn read_rows(self: &Rc<Self>) {
        let (targets, rolls, artist, copyright) = self.row_sources();
        let (reading, generation) = {
            let c = self.export.borrow();
            (c.reading.clone(), c.reading.fetch_add(1, Ordering::Relaxed) + 1)
        };
        std::thread::Builder::new()
            .name("export rows".into())
            .spawn(move || {
                let mut low = 0;
                for (i, (target, roll)) in targets.iter().zip(&rolls).enumerate() {
                    if reading.load(Ordering::Relaxed) != generation {
                        return;
                    }
                    let row = export::opening_row(&target.path, roll.as_ref(), &artist, &copyright);
                    low += usize::from(oni_decode::is_low_precision(&target.path));
                    with_shell(move |c| c.row_landed(generation, i, row));
                }
                with_shell(move |c| {
                    {
                        let mut card = c.export.borrow_mut();
                        if card.reading.load(Ordering::Relaxed) != generation {
                            return;
                        }
                        card.low_precision = low;
                    }
                    c.publish_export();
                });
            })
            .expect("export rows thread");
    }

    /// A line typed in before it lands keeps the typing.
    fn row_landed(self: &Rc<Self>, generation: u64, i: usize, row: Row) {
        {
            let mut c = self.export.borrow_mut();
            if c.reading.load(Ordering::Relaxed) != generation || c.rows.get(i).is_none_or(Option::is_some) {
                return;
            }
            c.rows[i] = Some(row);
        }
        self.publish_line(i);
    }

    /// Every line still on its way read here and now, as a write cannot wait for them.
    pub(super) fn rows_now(self: &Rc<Self>) {
        if self.export.borrow().rows.iter().all(Option::is_some) {
            return;
        }
        let (targets, rolls, artist, copyright) = self.row_sources();
        for (i, (target, roll)) in targets.iter().zip(&rolls).enumerate() {
            if self.export.borrow().rows.get(i).is_some_and(Option::is_none) {
                let row = export::opening_row(&target.path, roll.as_ref(), &artist, &copyright);
                self.export.borrow_mut().rows[i] = Some(row);
                self.publish_line(i);
            }
        }
    }

    /// The previews on `PREVIEW_WORKERS` threads, each taking the next frame in order, so a decode
    /// waits on no other frame's render; a newer pass stops them.
    pub(super) fn render_previews(self: &Rc<Self>, targets: Vec<Target>, generation: u64, pass: Arc<AtomicU64>) {
        let (engine, previews, screen, open) = {
            let s = self.state.borrow();
            (self.engine.clone(), s.previews.clone(), s.screen, s.negative.clone())
        };
        let room = self.export.borrow().room.clone();
        let (targets, next, asked) = (Arc::new(targets), Arc::new(std::sync::atomic::AtomicUsize::new(0)), std::time::Instant::now());
        for _ in 0..PREVIEW_WORKERS {
            let (engine, previews, open, room, pass, targets, next) = (engine.clone(), previews.clone(), open.clone(), room.clone(), pass.clone(), targets.clone(), next.clone());
            let live = move || pass.load(Ordering::Relaxed) == generation;
            std::thread::Builder::new()
                .name("export previews".into())
                .spawn(move || {
                    loop {
                        let i = next.fetch_add(1, Ordering::SeqCst);
                        let Some(target) = targets.get(i) else { return };
                        if !live() {
                            return;
                        }
                        let path = oni_library::paths::normal(&target.path.to_string_lossy()).into_owned();
                        let slide = crate::render::Reading::of(&target.settings).slide;
                        let known = open.as_ref().filter(|(p, _)| *p == path).map(|(_, n)| n.clone()).or_else(|| previews.lock().ok()?.get(&path, slide));
                        let negative = match known {
                            Some(n) => Ok(n),
                            None => prepared(&engine, &path, crate::render::Reading::of(&target.settings)),
                        };
                        let full = negative.as_ref().ok().map(|n| {
                            let (w, h) = n.size();
                            (n.full_width, n.full_width * f64::from(h) / f64::from(w.max(1)))
                        });
                        let size = full.map(|f| export::cropped_size(f, &target.settings));
                        // The size lands at once, as the size boxes wait for it; the render waits for the room.
                        with_shell(move |c| c.size_landed(generation, i, size));
                        while live() && asked.elapsed() < ROOM_WAIT && room.lock().is_ok_and(|r| r.is_none()) {
                            std::thread::sleep(std::time::Duration::from_millis(5));
                        }
                        let side = full.zip(size).zip(room.lock().ok().and_then(|r| *r)).map_or(PREVIEW_SIDE as usize, |((f, s), r)| preview_side(s, f.0.max(f.1), r));
                        // One exact render per photo in the export space, as the canvas shows it: proofed.
                        let picture = negative.and_then(|n| {
                            let linear = engine.lock()?.render(&n, &target.settings, Some(side))?;
                            let k = Params::new(screen, linear.width as u32, linear.height as u32, i as u32);
                            Ok((crate::render::encoded(&linear, &k, 1), linear.width, linear.height))
                        });
                        with_shell(move |c| {
                            if c.export.borrow().pass.load(Ordering::Relaxed) != generation {
                                return;
                            }
                            c.export.borrow_mut().previews[i] = match picture {
                                Ok((bytes, w, h)) => Preview::Ready(image(&bytes, w, h)),
                                Err(_) => Preview::Failed,
                            };
                            c.publish_line(i);
                        });
                    }
                })
                .expect("export preview thread");
        }
    }

    /// A target's size after its turns and crop, published and weighed when it changed.
    fn size_landed(self: &Rc<Self>, generation: u64, i: usize, size: Option<(f64, f64)>) {
        {
            let mut card = self.export.borrow_mut();
            if card.pass.load(Ordering::Relaxed) != generation || card.sizes[i] == size {
                return;
            }
            card.sizes[i] = size;
        }
        self.publish_line(i);
        if i == 0 {
            self.publish_export();
            self.weigh_export();
        }
    }

    /// `refreshEstimate`: the first target weighed on its 500 px sample, multiplied by the count; asked
    /// of the weigher, so a burst of changes weighs the latest once.
    pub(super) fn weigh_export(self: &Rc<Self>) {
        let (target, full, export, generation, weighing) = {
            let mut c = self.export.borrow_mut();
            c.estimate = None;
            let generation = c.weighing.fetch_add(1, Ordering::Relaxed) + 1;
            let (Some(target), Some(full)) = (c.targets.first().cloned(), c.full()) else { return };
            (target, full, c.settings.clone(), generation, c.weighing.clone())
        };
        let (engine, previews, open) = {
            let s = self.state.borrow();
            (self.engine.clone(), s.previews.clone(), s.negative.clone())
        };
        let path = oni_library::paths::normal(&target.path.to_string_lossy()).into_owned();
        let slide = crate::render::Reading::of(&target.settings).slide;
        let known = open.filter(|(p, _)| *p == path).map(|(_, n)| n).or_else(|| previews.lock().ok()?.get(&path, slide));
        self.export.borrow().weigher.ask(Weigh { target, path, known, full, export, generation, weighing, engine });
    }
}

/// One weighing asked: the first target, its preview when one is at hand, and the export's settings.
pub(super) struct Weigh {
    target: Target,
    path: String,
    known: Option<Arc<crate::render::Negative>>,
    full: (f64, f64),
    export: Settings,
    generation: u64,
    weighing: Arc<AtomicU64>,
    engine: Shared,
}

/// The weigher: the sample rendered once a frame and its settings, then each export's settings
/// weighed on it, which encodes 500 px and decodes nothing.
pub(super) fn weigher(runs: Arc<std::sync::atomic::AtomicUsize>) -> crate::latest::Worker<Weigh> {
    let mut kept: Option<(PathBuf, oni_settings::PipelineSettings, Option<oni_export::Image>)> = None;
    crate::latest::Worker::spawn("export estimate", move |w: Weigh| {
        if w.weighing.load(Ordering::Relaxed) != w.generation {
            return;
        }
        runs.fetch_add(1, Ordering::SeqCst);
        if kept.as_ref().is_none_or(|(p, s, _)| *p != w.target.path || *s != w.target.settings) {
            let negative = w.known.or_else(|| prepared(&w.engine, &w.path, crate::render::Reading::of(&w.target.settings)).ok());
            let sample = negative.and_then(|n| export::sample(&w.engine, &n, &w.target.settings));
            kept = Some((w.target.path.clone(), w.target.settings.clone(), sample));
        }
        let weighed = kept.as_ref().and_then(|(_, _, s)| s.as_ref()).map(|s| oni_export::estimate(s, w.full.0 * w.full.1, &w.export));
        let generation = w.generation;
        with_shell(move |c| {
            if c.export.borrow().weighing.load(Ordering::Relaxed) != generation {
                return;
            }
            c.export.borrow_mut().estimate = weighed;
            c.publish_export();
        });
    })
}

/// Previews rendered at once: their decodes take the decode gate's two places.
const PREVIEW_WORKERS: usize = crate::decode_gate::DECODES;

/// How long the previews wait for the column's room before rendering at Swift's side; the column
/// is laid out on the frame after the card shows.
const ROOM_WAIT: std::time::Duration = std::time::Duration::from_secs(1);

/// A line's preview room in pixels: the picture's width, the height under its caption, the slot's floor.
pub(super) type Room = (f64, f64, f64);

/// The long side of the negative a preview is rendered from: `cropped` (full pixels) shown as
/// `ExportColumn` lays it in `room`, never past Swift's `Export.previewSide`.
pub(super) fn preview_side(cropped: (f64, f64), full_long: f64, (width, height, floor): Room) -> usize {
    let aspect = if cropped.0 > 0.0 && cropped.1 > 0.0 { cropped.0 / cropped.1 } else { 1.5 };
    let slot = floor.max(height.min(width / aspect));
    let shown = if slot * aspect < width { slot.max(slot * aspect) } else { width.max(width / aspect) };
    let side = shown * full_long / cropped.0.max(cropped.1).max(1.0);
    (side.ceil() as usize).clamp(1, PREVIEW_SIDE as usize)
}

impl Ctl {
    /// The column laid out at a new room: kept for the renders to come, and every preview rendered
    /// again once the room grows past what they were drawn for.
    pub(super) fn preview_room(self: &Rc<Self>, room: Room) {
        let grown = {
            let c = self.export.borrow();
            let Ok(mut kept) = c.room.lock() else { return };
            let before = kept.replace(room);
            c.shown && before.is_some_and(|b| room.0 > b.0 || room.1 > b.1)
        };
        if grown {
            let (targets, generation, pass) = {
                let c = self.export.borrow();
                (c.targets.clone(), c.pass.fetch_add(1, Ordering::Relaxed) + 1, c.pass.clone())
            };
            self.render_previews(targets, generation, pass);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_preview_is_rendered_at_the_size_its_slot_shows_it() {
        let room = (1600.0, 1200.0, 320.0);
        // A 3:2 frame fills the width, a 2:3 one the height left under its caption.
        assert_eq!(preview_side((6000.0, 4000.0), 6000.0, room), 1600);
        assert_eq!(preview_side((4000.0, 6000.0), 6000.0, (1000.0, 1200.0, 320.0)), 1200);
        assert_eq!(preview_side((3000.0, 2000.0), 6000.0, (800.0, 600.0, 320.0)), 1600, "a crop to half asks twice the negative");
        assert_eq!(preview_side((4000.0, 6000.0), 6000.0, (400.0, 300.0, 320.0)), 320, "the floor holds a short slot up");
        // Twin: the crop ignored would render a half-cropped frame at half the slot's size.
        assert_eq!(preview_side((6000.0, 4000.0), 6000.0, (800.0, 600.0, 320.0)), 800);
        assert_eq!(preview_side((6000.0, 4000.0), 6000.0, (8000.0, 6000.0, 320.0)), PREVIEW_SIDE as usize, "never past Swift's side");
    }
}
