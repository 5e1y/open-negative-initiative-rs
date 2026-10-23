//! The screen-gamut stripes, which encode the canvas again at their own pace while on.

use super::*;

impl Ctl {
    /// `ScreenGamut`'s stripes on and off; while on the canvas is encoded again at their pace,
    /// from the render it holds, so the stripes move and the pipeline never runs for them.
    pub(in crate::app) fn toggle_screen_gamut(self: &Rc<Self>) {
        let on = {
            let mut s = self.state.borrow_mut();
            s.screen_gamut = !s.screen_gamut;
            s.screen_gamut
        };
        if on {
            let me = Rc::downgrade(self);
            let pace = Duration::from_secs_f64(1.0 / f64::from(oni_colour::gamut::FRAMES_PER_SECOND));
            self.stripes.start(TimerMode::Repeated, pace, move || {
                if let Some(me) = me.upgrade() {
                    me.restripe();
                }
            });
        } else {
            self.stripes.stop();
        }
        self.restripe();
    }

    /// The canvas, and Compare's before, encoded again from the renders they hold.
    pub(in crate::app) fn restripe(&self) {
        let Some(w) = self.window() else { return };
        let canvas = self.canvas_size();
        let (frame, before) = {
            let s = self.state.borrow();
            let (screen, g) = (s.screen, &s.gpu);
            let mut frame = None;
            if let (Some(source), Some(target)) = (&g.current, &g.target)
                && let Some(view) = source.view()
            {
                let e = source.extent();
                let k = zebra(&s, Params::new(screen, e.w, e.h, g.seed), s.placed, e.w, canvas);
                g.kernel.run(&g.device, &g.queue, view, &k, target);
                frame = g.read_back(target, (e.w, e.h));
            }
            let mut read = None;
            if let Some((target, before)) = &g.before
                && let Some(view) = before.view()
            {
                let e = before.extent();
                let k = zebra(&s, Params::new(screen, e.w, e.h, g.seed), s.compare.before_placed, e.w, canvas);
                g.kernel.run(&g.device, &g.queue, view, &k, target);
                read = g.read_back(target, (e.w, e.h));
            }
            (frame, read)
        };
        let c = w.global::<Canvas>();
        if let Some(image) = frame {
            c.set_frame(image);
        }
        if let Some(image) = before {
            c.set_before(image);
        }
        w.window().request_redraw();
    }
}
