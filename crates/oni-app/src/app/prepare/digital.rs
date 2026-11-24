//! A digital roll: every photo of the roll made digital as one step, each on its film's own reference, the windows'
//! white where the roll's lights lie, read off this thread as a scanned roll's are; the bare film, known, is the black.

use super::*;

impl Ctl {
    /// PREPARE ROLL on Digital: each photo's lights read off this thread with the measure's progress and Stop, then
    /// laid once every read and draft write queued before it is done; Stop leaves every frame as it was.
    pub(super) fn prepare_digital(self: &Rc<Self>) {
        let job = {
            let mut s = self.state.borrow_mut();
            let open = s.open_path().map(str::to_string);
            let live = s.store.settings().clone();
            let r = &mut s.prepare;
            if r.sources.is_empty() || r.progress.is_some() {
                return;
            }
            r.stop.store(false, Ordering::Relaxed);
            r.progress = Some((0, r.sources.len()));
            let (stop, session) = (r.stop.clone(), r.session);
            let paths: Vec<String> = r.sources.iter().map(|f| f.path.clone()).collect();
            let job: Job = Box::new(move || {
                let m = crate::measuring::measurer();
                let mut lights = Vec::new();
                for path in &paths {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    let before = if Some(path) == open.as_ref() {
                        Some(live.clone())
                    } else {
                        sidecar::read(Path::new(path), mosaic(path)).ok().map(|l| l.settings)
                    };
                    let start = before.map(|b| oni_core::store::roll::digital(&b, 1.0));
                    if let Some(light) = start.and_then(|start| m?.with(path, &start, |r, copy| key::digital_lights(r, copy, &start))) {
                        lights.push(light);
                    }
                    let landed = trace::Landed { name: oni_library::paths::name(path), frame: None, roll: None };
                    with_shell(move |c| c.measure_progressed(session, landed));
                }
                let lights = roll::digital_lights(lights).unwrap_or(1.0);
                with_shell(move |c| c.land_digital(session, lights));
            });
            job
        };
        self.state.borrow().prepare.post(job);
        self.publish_prepare();
        self.publish_bar();
    }

    /// Every file of the roll as it came made digital on the roll's `lights`, the passes a grouping only proposed each
    /// on its own, since no measure registers them; then the roll landed as a measure lands.
    fn land_digital(self: &Rc<Self>, session: u64, lights: f32) {
        let (paths, stopped) = {
            let r = &self.state.borrow().prepare;
            (r.sources.iter().map(|f| f.path.clone()).collect::<Vec<String>>(), r.stop.load(Ordering::Relaxed))
        };
        if self.prepare_ended(session).is_none() {
            return;
        }
        if stopped {
            self.publish_prepare();
            self.publish_bar();
            return;
        }
        let failure = {
            let mut s = self.state.borrow_mut();
            s.store.make_digital(&paths, lights);
            s.store.failure.take()
        };
        self.land_prepared(&paths, &paths, paths.clone(), failure, |_| {});
    }
}
