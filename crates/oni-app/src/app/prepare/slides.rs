//! A slide roll: never measured, so Prepare the Roll walks its film and flat-field alone and writes
//! each frame on the roll unmeasured (owner's call).

use super::*;

/// The step whose key ends the roll: the window's measure, or a slide's last step.
pub fn last_step(capture: Capture, film: ConversionMode) -> Step {
    Step::ALL.into_iter().rev().find(|s| !s.skipped(capture, film)).unwrap_or(Step::Film)
}

/// A slide roll's key: its frames written on the roll, unmeasured.
pub fn slides_title(n: usize) -> String {
    trn("PREPARE {n} FRAME", "PREPARE {n} FRAMES", n as u64)
}

impl Roll {
    /// The roll's film, its step brought back inside the steps that film walks.
    pub(super) fn set_film(&mut self, film: ConversionMode) {
        self.film = film;
        if self.step.skipped(self.capture, film) {
            self.step = last_step(self.capture, film);
        }
    }
}

impl Ctl {
    /// A slide roll: every ticked frame written on the roll's film and flat-field off this thread,
    /// unmeasured, then landed as a measure lands (owner's call).
    pub(super) fn prepare_slides(self: &Rc<Self>) {
        let job = {
            let mut s = self.state.borrow_mut();
            let open = s.open.clone();
            let live = s.store.settings().clone();
            let r = &mut s.prepare;
            let picked: Vec<String> = r.frames.iter().filter(|f| f.ticked).map(|f| f.path.clone()).collect();
            if picked.is_empty() || r.progress.is_some() {
                return;
            }
            r.progress = Some((0, picked.len()));
            let (mask, session) = (r.mask(), r.session);
            let job: Job = Box::new(move || {
                let asked = picked.len();
                let common = CaptureExposure::common(&picked.iter().map(|p| CaptureExposure::read(Path::new(p))).collect::<Vec<_>>());
                let stops = |path: &str| match (common, CaptureExposure::read(Path::new(path))) {
                    (Some(c), Some(own)) => own.stops() - c.stops(),
                    _ => 0.0,
                };
                let written: Written = picked
                    .iter()
                    .filter_map(|path| {
                        let before = if Some(path) == open.as_ref() {
                            live.clone()
                        } else {
                            match sidecar::read(Path::new(path), mosaic(path), true) {
                                Ok(l) if l.writable => l.settings,
                                _ => return None,
                            }
                        };
                        let after = roll::slide(&before, mask.as_ref(), stops(path));
                        Some((path.clone(), before, after))
                    })
                    .collect();
                with_shell(move |c| c.land_measure(session, Some(written), None, false, asked));
            });
            job
        };
        self.state.borrow().prepare.post(job);
        self.publish_prepare();
        self.publish_bar();
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slide_roll_stops_at_the_flat_field_with_no_measure() {
        let p = ConversionMode::Positive;
        assert_eq!(neighbour(Step::Film, 1, Capture::Camera, p), Step::FlatField);
        assert_eq!(neighbour(Step::FlatField, 1, Capture::Camera, p), Step::FlatField, "no base, flags or window");
        assert_eq!(neighbour(Step::Film, 1, Capture::Scanner, p), Step::Film, "a scanner's slides have the film alone");
        assert_eq!((last_step(Capture::Camera, p), last_step(Capture::Scanner, p)), (Step::FlatField, Step::Film));
        assert!([Step::Base, Step::Flags, Step::Window].iter().all(|s| s.skipped(Capture::Camera, p)));
        // Twin: a negative roll still walks to its measure.
        let n = ConversionMode::Negative;
        assert_eq!(neighbour(Step::FlatField, 1, Capture::Camera, n), Step::Base);
        assert!(![Step::Base, Step::Flags, Step::Window].iter().any(|s| s.skipped(Capture::Camera, n)));
    }

    #[test]
    fn a_roll_turned_to_slides_leaves_the_steps_they_skip() {
        let mut r = Roll { step: Step::Flags, capture: Capture::Camera, ..Roll::default() };
        r.set_film(ConversionMode::Positive);
        assert_eq!(r.step, Step::FlatField, "a slide roll has no flags step");
        // Twin: a negative roll keeps its step.
        let mut n = Roll { step: Step::Flags, capture: Capture::Camera, ..Roll::default() };
        n.set_film(ConversionMode::Negative);
        assert_eq!(n.step, Step::Flags);
    }
}
