//! A slide roll: one measuring window for the roll, then each frame's own gains aligned through it,
//! never a roll reference (owner's call).

use super::*;
use super::trace::{BINS, Drawn};

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
        if self.step.skipped(self.capture, self.walked()) {
            self.step = Step::Window;
        }
    }
}

impl Ctl {
    /// A slide roll: every ticked frame on the roll's film, window and flat-field, levelled on its own
    /// off this thread, landed as one step; a blank slide is left unlevelled.
    pub(super) fn prepare_slides(self: &Rc<Self>) {
        let job = {
            let mut s = self.state.borrow_mut();
            let open = s.open.clone();
            let live = s.store.settings().clone();
            let screen = s.screen;
            let r = &mut s.prepare;
            let picked: Vec<String> = r.frames.iter().filter(|f| f.ticked).map(|f| f.path.clone()).collect();
            if picked.is_empty() || r.progress.is_some() {
                return;
            }
            r.stop.store(false, Ordering::Relaxed);
            r.progress = Some((0, picked.len()));
            let (window, mask, stop, session) = (r.window, r.mask(), r.stop.clone(), r.session);
            let job: Job = Box::new(move || {
                let common = CaptureExposure::common(&picked.iter().map(|p| CaptureExposure::read(Path::new(p))).collect::<Vec<_>>());
                let stops = |path: &str| match (common, CaptureExposure::read(Path::new(path))) {
                    (Some(c), Some(own)) => own.stops() - c.stops(),
                    _ => 0.0,
                };
                let m = crate::measuring::measurer();
                let side = oni_measure::sheet::roll_tile();
                let mut written: Written = Vec::new();
                let mut blank = Vec::new();
                let mut roll = SlideRoll::default();
                for path in &picked {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    let before = if Some(path) == open.as_ref() {
                        Some(live.clone())
                    } else {
                        sidecar::read(Path::new(path), mosaic(path), true).ok().filter(|l| l.writable).map(|l| l.settings)
                    };
                    let mut drawn = None;
                    let mut voted = false;
                    if let Some(before) = before {
                        let start = roll::slide(&before, window, mask.as_ref(), stops(path));
                        let seen = uncropped(&start);
                        let read = m.and_then(|m| m.with(path, &seen, |r, copy| Some((key::slide_read(r, copy, &seen), oni_measure::sheet::tile(r, copy, &seen, side)))));
                        if let Some((read, tile)) = read {
                            drawn = tile.as_ref().and_then(|t| Drawn::of(t, screen).zip(negative::of(t)));
                            match read {
                                Some((settings, reading)) => {
                                    roll.add(&reading.rgb);
                                    voted = true;
                                    // Measured whole, written on the frame's own crop.
                                    written.push((path.clone(), before, PipelineSettings { geometry: start.geometry, ..settings }));
                                }
                                None => {
                                    blank.push(path.clone());
                                    written.push((path.clone(), before, start));
                                }
                            }
                        }
                    }
                    let shown = voted.then(|| roll.drawn()).flatten();
                    let landed = trace::Landed { name: oni_library::paths::name(path), frame: drawn, roll: shown };
                    with_shell(move |c| c.measure_progressed(session, landed));
                }
                let stopped = stop.load(Ordering::Relaxed);
                let written = (!stopped).then_some(written);
                with_shell(move |c| c.land_measure(session, written, None, stopped, &picked, &blank));
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
    fn a_slide_roll_walks_its_film_flat_field_and_window_alone() {
        let p = ConversionMode::Positive;
        assert_eq!(neighbour(Step::Film, 1, Capture::Camera, p), Step::FlatField);
        assert_eq!(neighbour(Step::FlatField, 1, Capture::Camera, p), Step::Window, "no base and no flags");
        assert_eq!(neighbour(Step::Window, -1, Capture::Camera, p), Step::FlatField);
        assert_eq!(neighbour(Step::Film, 1, Capture::Scanner, p), Step::Window, "a scanner's slides skip the flat-field");
        assert!([Step::Base, Step::Flags].iter().all(|s| s.skipped(Capture::Camera, p)) && !Step::Window.skipped(Capture::Camera, p));
        assert_ne!(Step::Window.detail(p), Step::Window.detail(ConversionMode::Negative));
        // Twin: a negative roll still walks its base and flags.
        let n = ConversionMode::Negative;
        assert_eq!(neighbour(Step::FlatField, 1, Capture::Camera, n), Step::Base);
        assert!(![Step::Base, Step::Flags, Step::Window].iter().any(|s| s.skipped(Capture::Camera, n)));
    }

    #[test]
    fn a_roll_turned_to_slides_leaves_the_steps_they_skip_and_its_flags_count_no_more() {
        let mut r = Roll { step: Step::Flags, capture: Capture::Camera, positive_inverted: false, ..Roll::default() };
        r.set_film(ConversionMode::Positive);
        assert_eq!(r.step, Step::Window, "a slide roll on its gains alone has no flags step");
        // Twin: read as its own negative, it walks a negative's flags.
        let mut inverted = Roll { step: Step::Flags, capture: Capture::Camera, ..Roll::default() };
        inverted.set_film(ConversionMode::Positive);
        assert_eq!((inverted.step, inverted.walked()), (Step::Flags, ConversionMode::Negative));
        let mut frames = vec![Frame::new("a", ""), Frame::new("b", "")];
        frames[1].flagged = true;
        assert_eq!(measured(&frames, ConversionMode::Positive).len(), 2, "a flag a negative draft left is not a slide's");
        // Twin: a negative roll keeps its step and leaves its flagged frame out of the measure.
        let mut n = Roll { step: Step::Flags, capture: Capture::Camera, ..Roll::default() };
        n.set_film(ConversionMode::Negative);
        assert_eq!(n.step, Step::Flags);
        assert_eq!(measured(&frames, ConversionMode::Negative).len(), 1);
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
