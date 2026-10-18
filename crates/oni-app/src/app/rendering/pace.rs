//! What runs on its own pace beside the renders: the histogram, the load counter, the deferred write, the status line.

use super::*;

impl Ctl {
    /// The histogram and the load counter, on their own pace so a notch touches neither.
    pub(in crate::app) fn pulse(self: &Rc<Self>) {
        let Some(w) = self.window() else { return };
        // A tab opened or a full frame landed reaches the loupe at this pace when no render asks.
        self.request_loupe();
        let (histogram, loading) = {
            let mut s = self.state.borrow_mut();
            if let Some(h) = s.gpu.as_mut().and_then(Gpu::landed) {
                s.histogram = Some(h);
                s.histogram_dirty = true;
            }
            let histogram = if s.histogram_dirty {
                s.histogram_dirty = false;
                s.histogram.clone()
            } else {
                None
            };
            (histogram, s.load_bar())
        };
        if let Some(h) = histogram {
            let (pw, ph) = HISTOGRAM;
            w.global::<Plots>().set_output(premultiplied(&h.raster(pw, ph), pw, ph));
        }
        if w.global::<App>().get_gallery() {
            w.global::<Library>().set_loading(loading.unwrap_or_default().into());
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
