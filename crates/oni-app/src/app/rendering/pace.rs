//! What runs on its own pace beside the renders: the histogram, the load counter, the deferred write, the status line.

use super::*;
use super::plots::Slot;
use std::sync::atomic::AtomicU64;

/// The last histogram trace asked, so an older one landing after it is dropped.
static TRACES: AtomicU64 = AtomicU64::new(0);

impl Ctl {
    /// The load counter on its own pace, and the histogram should a count have waited for it.
    pub(in crate::app) fn pulse(self: &Rc<Self>) {
        let Some(w) = self.window() else { return };
        // A tab opened or a full frame landed reaches the loupe and De-dust at this pace when no render asks.
        self.request_loupe();
        self.ready_full();
        self.ask_originals();
        self.publish_histogram();
        self.watch_frontier_look();
        let loading = self.state.borrow().load_bar();
        let lib = w.global::<Library>();
        lib.set_loading(loading.as_ref().map(|b| b.title.clone()).unwrap_or_default().into());
        lib.set_loading_count(loading.as_ref().map(LoadBar::count).unwrap_or_default().into());
        lib.set_loading_fraction(loading.as_ref().map_or(-1.0, LoadBar::fraction));
    }

    /// A picture waiting for its count is counted once the image's cadence allows; a count read back
    /// publishes itself, so nothing polls the device meanwhile.
    pub(in crate::app) fn watch_histogram(&self) {
        let Some(wait) = self.state.borrow().gpu.count_wait() else { return };
        if self.histogram_watch.running() {
            return;
        }
        self.histogram_watch.start(TimerMode::SingleShot, wait.max(MIN_WAIT), move || {
            let Some(me) = SHELL.with(|s| s.borrow().upgrade()) else { return };
            me.publish_histogram();
        });
    }

    /// The histogram traced into the panel once a count has landed.
    pub(in crate::app) fn publish_histogram(&self) {
        let Some(w) = self.window() else { return };
        let (histogram, counted) = {
            let mut s = self.state.borrow_mut();
            let landed = s.gpu.landed();
            drop(s);
            self.watch_histogram();
            let mut s = self.state.borrow_mut();
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
            let scale = w.window().scale_factor();
            let under = crate::editors::size(self, crate::editors::Plot::Saturation);
            let sizes = [(f64::from(plots.get_output_width()), f64::from(plots.get_output_height())), (under.w, under.h)];
            // Traced off this thread, 1.5 ms a plot; a trace overtaken by a later one is dropped.
            let (ticket, writer) = (TRACES.fetch_add(1, Ordering::SeqCst) + 1, self.plot_writer());
            super::plots::plot(move |p| {
                if TRACES.load(Ordering::SeqCst) != ticket {
                    return;
                }
                let [output, saturation] = [Slot::Output, Slot::Saturation].map(|slot| p.trace(&writer, slot, &h, None, sizes[slot as usize], scale));
                with_shell(move |c| {
                    let Some(w) = c.window() else { return };
                    super::plots::land(&w, Slot::Output, output);
                    super::plots::land(&w, Slot::Saturation, saturation);
                    if let Some(at) = counted {
                        c.state.borrow_mut().notches.record_histogram(at.elapsed());
                    }
                });
            });
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
