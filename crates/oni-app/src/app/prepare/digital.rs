//! A digital roll: no film to measure, every photo of the roll made digital as one step, each white
//! resting on its own As Shot.

use super::*;

impl Ctl {
    /// PREPARE ROLL on Digital: laid once every read and draft write queued before it is done, as
    /// those may still rewrite a frame's sidecar; Stop until then leaves every frame as it was.
    pub(super) fn prepare_digital(self: &Rc<Self>) {
        let session = {
            let r = &mut self.state.borrow_mut().prepare;
            if r.sources.is_empty() || r.progress.is_some() {
                return;
            }
            r.stop.store(false, Ordering::Relaxed);
            r.progress = Some((0, r.sources.len()));
            r.session
        };
        self.state.borrow().prepare.post(Box::new(move || with_shell(move |c| c.land_digital(session))));
        self.publish_prepare();
        self.publish_bar();
    }

    /// Every file of the roll as it came made digital, the passes a grouping only proposed each on its
    /// own, since no measure registers them; then the roll landed as a measure lands.
    fn land_digital(self: &Rc<Self>, session: u64) {
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
            s.store.make_digital(&paths);
            s.store.failure.take()
        };
        self.land_prepared(&paths, &paths, paths.clone(), failure, |_| {});
    }
}
