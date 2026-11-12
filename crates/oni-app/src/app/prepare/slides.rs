//! A slide roll: one measuring window for the roll, one set of gains from the white picked, none
//! without a pick, never a roll reference (owner's call).

use super::*;
use super::trace::{BINS, Drawn};
use oni_settings::blocks::Gains;

/// A slide roll's screen so far: its slides' readings summed, no ends, since a slide keeps its windows
/// at rest and moves its gains alone.
#[derive(Default)]
pub(in crate::app) struct SlideRoll {
    sum: Vec<[f32; 3]>,
    slides: usize,
}

impl SlideRoll {
    pub fn add(&mut self, counts: &[[f32; 3]]) {
        if self.sum.is_empty() {
            self.sum = vec![[0.0; 3]; BINS];
        }
        let last = counts.len().saturating_sub(1).max(1) as f32;
        for (i, b) in counts.iter().enumerate() {
            let at = ((i as f32 / last * BINS as f32) as usize).min(BINS - 1);
            (0..3).for_each(|c| self.sum[at][c] += b[c]);
        }
        self.slides += 1;
    }

    pub fn drawn(&self) -> Option<(Drawn, usize)> {
        (self.slides > 0).then(|| (Drawn::stacked((0..3).map(|c| self.sum.iter().map(|b| b[c]).collect()).collect(), None), self.slides))
    }
}

impl Roll {
    /// The roll's film, its step brought back inside the steps that film walks.
    pub(super) fn set_film(&mut self, film: ConversionMode) {
        self.film = film;
        if self.step.skipped(self.capture, self.film) {
            self.step = Step::Window;
        }
    }
}

impl Ctl {
    /// A slide roll: the white picked read once, its gains laid on every ticked slide, none when no
    /// white was picked; each slide read off this thread for the roll's screen and its blank frames.
    pub(super) fn prepare_slides(self: &Rc<Self>) {
        let job = {
            let mut s = self.state.borrow_mut();
            let open = s.open.clone();
            let live = s.store.settings().clone();
            let screen = s.screen;
            let r = &mut s.prepare;
            let picked: Vec<Frame> = r.frames.iter().filter(|f| f.ticked).cloned().collect();
            if picked.is_empty() || r.progress.is_some() {
                return;
            }
            r.stop.store(false, Ordering::Relaxed);
            r.progress = Some((0, picked.len()));
            let (window, mask, pick, stop, session) = (r.window, r.mask(), r.base.clone(), r.stop.clone(), r.session);
            let job: Job = Box::new(move || {
                let paths: Vec<String> = picked.iter().map(|f| f.path.clone()).collect();
                let common = CaptureExposure::common(&paths.iter().map(|p| CaptureExposure::read(Path::new(p))).collect::<Vec<_>>());
                let stops = |path: &str| match (common, CaptureExposure::read(Path::new(path))) {
                    (Some(c), Some(own)) => own.stops() - c.stops(),
                    _ => 0.0,
                };
                let m = crate::measuring::measurer();
                let film = ConversionMode::Positive;
                // The white read first, its colour alone kept, on the start its slide is read from.
                let white = pick.and_then(|p| {
                    let before = if Some(&p.path) == open.as_ref() {
                        live.clone()
                    } else {
                        sidecar::read(Path::new(&p.path), mosaic(&p.path), true).map(|l| l.settings).unwrap_or_default()
                    };
                    m?.base(&p.path, &roll::slide(&before, window, mask.as_ref(), stops(&p.path)), Point::new(p.unit.0, p.unit.1), p.radius)
                });
                let gains = white.and_then(|w| oni_measure::slide_gains::white(w, film));
                let side = oni_measure::tiles::roll_tile();
                let mut written: Written = Vec::new();
                let mut blank = Vec::new();
                let mut roll = SlideRoll::default();
                for f in &picked {
                    let path = &f.path;
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    let before = if Some(path) == open.as_ref() {
                        Some(live.clone())
                    } else {
                        sidecar::read(Path::new(path), mosaic(path), true).ok().filter(|l| l.writable).map(|l| l.settings)
                    };
                    let mut drawn = None;
                    let mut read_one = false;
                    if let Some(before) = before {
                        let start = roll::slide(&before, window, mask.as_ref(), stops(path));
                        let seen = uncropped(&start);
                        let read = m.and_then(|m| m.with(path, &seen, |r, copy| Some((key::slide_read(r, copy, &seen), oni_measure::tiles::tile(r, copy, &seen, side)))));
                        if let Some((read, tile)) = read {
                            drawn = tile.as_ref().and_then(|t| Drawn::of(t, screen).zip(negative::of(t)));
                            match read {
                                Some(read) => {
                                    roll.add(&read.reading.rgb);
                                    read_one = true;
                                }
                                None => blank.push(path.clone()),
                            }
                            written.push((path.clone(), before, start));
                        }
                    }
                    let shown = read_one.then(|| roll.drawn()).flatten();
                    let landed = trace::Landed { name: oni_library::paths::name(path), frame: drawn, roll: shown };
                    with_shell(move |c| c.measure_progressed(session, landed));
                }
                let stopped = stop.load(Ordering::Relaxed);
                // The white's gains, else the camera's 5500 K at rest: the base each Source Balance rests on.
                let stops = gains.unwrap_or([Gains::BASE; 3]);
                let balance = oni_measure::auto_levels::Balance { stops, levels: oni_measure::auto_levels::resting(film), shift: [0.0; 3] };
                for (_, _, s) in &mut written {
                    *s = oni_core::balance::levelled(&balance.core(), s);
                    s.measured_gains = gains;
                }
                let written = (!stopped).then_some(written);
                with_shell(move |c| c.land_measure(session, written, stopped, &paths, &blank));
            });
            job
        };
        self.state.borrow().prepare.post(job);
        self.trace_cleared();
        self.publish_prepare();
        self.publish_bar();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slide_roll_walks_its_film_flat_field_white_and_window() {
        let p = ConversionMode::Positive;
        assert_eq!(neighbour(Step::Film, 1, Capture::Camera, p), Step::FlatField);
        assert_eq!(neighbour(Step::FlatField, 1, Capture::Camera, p), Step::Base, "the white is picked on the base step");
        assert_eq!(neighbour(Step::Base, 1, Capture::Camera, p), Step::Window, "no flags: nothing votes");
        assert_eq!(neighbour(Step::Window, -1, Capture::Camera, p), Step::Base);
        assert_eq!(neighbour(Step::Film, 1, Capture::Scanner, p), Step::Base, "a scanner's slides skip the flat-field");
        assert!(Step::Flags.skipped(Capture::Camera, p) && ![Step::Base, Step::Window].iter().any(|s| s.skipped(Capture::Camera, p)));
        assert_ne!(Step::Window.detail(p), Step::Window.detail(ConversionMode::Negative));
        assert_ne!((Step::Base.title(p), Step::Base.detail(p)), (Step::Base.title(ConversionMode::Negative), Step::Base.detail(ConversionMode::Negative)));
        // Twin: a negative roll still walks its base.
        let n = ConversionMode::Negative;
        assert_eq!(neighbour(Step::FlatField, 1, Capture::Camera, n), Step::Base);
        assert!(![Step::Base, Step::Flags, Step::Window].iter().any(|s| s.skipped(Capture::Camera, n)));
    }

    #[test]
    fn a_roll_turned_to_slides_keeps_its_base_step_for_the_white_and_leaves_its_flags() {
        let mut r = Roll { step: Step::Base, capture: Capture::Camera, ..Roll::default() };
        r.set_film(ConversionMode::Positive);
        assert_eq!(r.step, Step::Base, "a slide roll picks its white on the base step");
        let mut flags = Roll { step: Step::Flags, capture: Capture::Camera, ..Roll::default() };
        flags.set_film(ConversionMode::Positive);
        assert_eq!(flags.step, Step::Window, "a slide roll has no flags step");
        // Twin: a negative roll keeps its flags step.
        let mut negative = Roll { step: Step::Flags, capture: Capture::Camera, ..Roll::default() };
        negative.set_film(ConversionMode::Negative);
        assert_eq!(negative.step, Step::Flags);
        let mut frames = vec![Frame::new("a", ""), Frame::new("b", "")];
        frames[1].flagged = true;
        assert_eq!(measured(&frames).iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["a"], "a flagged slide votes not");
    }

    #[test]
    fn a_slide_roll_sums_its_slides_and_places_no_ends() {
        let mut roll = SlideRoll::default();
        assert!(roll.drawn().is_none(), "dark until a slide lands");
        let counts = |at: usize| (0..1024).map(|i| if i == at { [1.0; 3] } else { [0.0; 3] }).collect::<Vec<_>>();
        roll.add(&counts(100));
        roll.add(&counts(900));
        roll.add(&counts(900));
        let (d, n) = roll.drawn().unwrap();
        assert_eq!((n, d.ends), (3, None), "a slide's windows stay at rest");
        assert!(d.bins[100 * BINS / 1024][0] > 0.0 && d.bins[900 * BINS / 1024][0] == 1.0, "both readings summed, the tallest at the top");
        // Twin: one slide alone shows its own reading only.
        let mut one = SlideRoll::default();
        one.add(&counts(100));
        assert_eq!(one.drawn().unwrap().0.bins[900 * BINS / 1024][0], 0.0);
    }
}
