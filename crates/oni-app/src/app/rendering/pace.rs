//! What runs on its own pace beside the renders: the histogram, the load counter, the deferred write, the status line.

use super::*;

impl Ctl {
    /// The load counter on its own pace, and the histogram should a count have waited for it.
    pub(in crate::app) fn pulse(self: &Rc<Self>) {
        let Some(w) = self.window() else { return };
        // A tab opened or a full frame landed reaches the loupe at this pace when no render asks.
        self.request_loupe();
        self.publish_histogram();
        let loading = self.state.borrow().load_bar();
        let lib = w.global::<Library>();
        lib.set_loading(loading.as_ref().map(|b| b.title.clone()).unwrap_or_default().into());
        lib.set_loading_count(loading.as_ref().map(LoadBar::count).unwrap_or_default().into());
        lib.set_loading_fraction(loading.as_ref().map_or(-1.0, LoadBar::fraction));
    }

    /// Polls the histogram's readback every `HISTOGRAM_POLL` while a count is under way, so a
    /// slide's histogram reaches the panel within a frame of its picture; stops once none is.
    pub(in crate::app) fn watch_histogram(&self) {
        if self.histogram_watch.running() {
            return;
        }
        self.histogram_watch.start(TimerMode::Repeated, HISTOGRAM_POLL, move || {
            let Some(me) = SHELL.with(|s| s.borrow().upgrade()) else { return };
            me.publish_histogram();
            if !me.state.borrow().gpu.as_ref().is_some_and(Gpu::histogram_pending) {
                me.histogram_watch.stop();
            }
        });
    }

    /// The histogram traced into the panel once a count has landed.
    pub(in crate::app) fn publish_histogram(&self) {
        let Some(w) = self.window() else { return };
        let (histogram, counted) = {
            let mut s = self.state.borrow_mut();
            let landed = s.gpu.as_mut().and_then(Gpu::landed);
            let counted = landed.as_ref().map(|(_, at)| *at);
            if let Some((h, _)) = landed {
                s.histogram = Some(h);
                s.histogram_dirty = true;
            }
            if !s.histogram_dirty {
                return;
            }
            s.histogram_dirty = false;
            (s.histogram.clone(), counted)
        };
        if let Some(h) = histogram {
            let plots = w.global::<Plots>();
            plots.set_output(self.traced(&h, None, f64::from(plots.get_output_width()), f64::from(plots.get_output_height())));
            let under = crate::editors::size(self, crate::editors::Plot::Saturation);
            plots.set_saturation(self.traced(&h, None, under.w, under.h));
        }
        if let Some(at) = counted {
            self.state.borrow_mut().notches.record_histogram(at.elapsed());
        }
    }

    /// The deferred write, `WRITE_DELAY` after the last change; a hand still on a slider puts it off
    /// until its release, which asks again.
    pub(in crate::app) fn schedule_write(self: &Rc<Self>) {
        let me = Rc::downgrade(self);
        self.writer.start(TimerMode::SingleShot, WRITE_DELAY, move || {
            let Some(me) = me.upgrade() else { return };
            let failure = {
                let mut s = me.state.borrow_mut();
                if s.slides.iter().any(|sl| sl.live.is_some()) {
                    return;
                }
                s.last_change = None;
                s.store.flush()
            };
            me.journal_settled();
            match failure {
                Some(message) => me.say(&message),
                None => me.schedule_redraw(),
            }
        });
    }

    /// Writes the open frame's sidecar now when the disk differs.
    pub(crate) fn flush(&self) {
        let failure = self.state.borrow_mut().store.flush();
        if let Some(message) = failure {
            self.say(&message);
        }
    }

    pub(in crate::app) fn say(&self, text: &str) {
        self.state.borrow_mut().status = text.to_string();
        if let Some(w) = self.window() {
            w.global::<Canvas>().set_status(text.into());
        }
    }
}
