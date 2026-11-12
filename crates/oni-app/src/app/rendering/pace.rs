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
        self.detection_due();
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
        let Some(wait) = ({
            let s = self.state.borrow();
            s.gpu.count_wait(&s.requests.count)
        }) else {
            return;
        };
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
            let landed = {
                let mut guard = self.state.borrow_mut();
                let s = &mut *guard;
                s.gpu.landed(&mut s.requests.count)
            };
            self.watch_histogram();
            let mut s = self.state.borrow_mut();
            let counted = landed.as_ref().map(|(_, at)| *at);
            if let Some((h, _)) = landed {
                s.requests.histogram = Some(h);
                s.histogram_dirty = true;
            }
            if !s.histogram_dirty {
                return;
            }
            s.histogram_dirty = false;
            (s.requests.histogram.clone(), counted)
        };
        // Only the plots on screen are traced; one hidden is traced again as it shows.
        let shown = self.picture_plots();
        if let Some(h) = histogram.filter(|_| shown.contains(&true)) {
            let plots = w.global::<Plots>();
            let scale = w.window().scale_factor();
            let under = crate::editors::size(self, crate::editors::Plot::Saturation);
            let sizes = [(f64::from(plots.get_output_width()), f64::from(plots.get_output_height())), (under.w, under.h)];
            // Traced off this thread, 1.5 ms a plot; a trace overtaken by a later one is dropped.
            let (ticket, writer) = (TRACES.fetch_add(1, Ordering::SeqCst) + 1, self.plot_writer());
            let since = [Slot::Output, Slot::Saturation].map(super::plots::clears);
            super::plots::plot(move |p| {
                if TRACES.load(Ordering::SeqCst) != ticket {
                    return;
                }
                let traced = [Slot::Output, Slot::Saturation].map(|slot| shown[slot as usize].then(|| p.trace(&writer, slot, &h, None, sizes[slot as usize], scale)));
                with_shell(move |c| {
                    let Some(w) = c.window() else { return };
                    for (slot, traced) in [Slot::Output, Slot::Saturation].into_iter().zip(traced) {
                        if let Some(traced) = traced {
                            super::plots::land_since(&w, slot, since[slot as usize], traced);
                        }
                    }
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
            // A gesture moving files may be moving this frame's: the write waits for its landing,
            // which follows the frame to its new path, or it would land beside the old one.
            if me.state.borrow().org.disk_busy.is_some() {
                return me.schedule_write();
            }
            let failure = {
                let mut s = me.state.borrow_mut();
                if s.slides.iter().any(|sl| sl.live().is_some()) {
                    return;
                }
                s.last_change = None;
                s.store.flush()
            };
            me.journal_settled();
            me.publish_saved();
            match failure {
                Some(message) => me.say(&message),
                None => me.schedule_redraw(),
            }
            me.balance_at_rest();
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
        let words = oni_i18n::spoken(text);
        if !text.is_empty() && self.state.borrow().status != words {
            crate::timing::shown("status", text);
        }
        if let Some(w) = self.window() {
            w.global::<Canvas>().set_status(words.as_str().into());
        }
        self.state.borrow_mut().status = words;
    }
}
