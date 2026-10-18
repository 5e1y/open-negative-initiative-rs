//! `DSScrollAdjust` on a slider: a wheel's lines or a touch surface's points along the track, the
//! hand held from the first event until `DSScroll.idleWindow` passes without one, one step in all.

use super::Ctl;
use crate::track::{self, SCROLL_IDLE};
use crate::viewport;
use slint::TimerMode;
use std::rc::Rc;
use std::sync::atomic::Ordering;

impl Ctl {
    /// The travel one scroll event asks of a track, from the platform's own delta when the window
    /// hook noted it; Slint's points stand for a touch surface otherwise.
    pub(super) fn scroll_travel(dx: f32) -> f32 {
        match viewport::take_wheel() {
            Some(w) if !w.precise => track::scroll_travel(w.dx as f32, false),
            _ => track::scroll_travel(dx, true),
        }
    }

    /// `CatcherView.keepAlive`: the hand raised at the first event, lowered once events stop.
    pub(super) fn scroll_kept_alive(self: &Rc<Self>) {
        {
            let mut s = self.state.borrow_mut();
            if !s.scrolling {
                s.scrolling = true;
                s.hands.fetch_add(1, Ordering::Relaxed);
                s.store.hold();
            }
        }
        let me = Rc::downgrade(self);
        self.scroll_idle.start(TimerMode::SingleShot, SCROLL_IDLE, move || {
            if let Some(me) = me.upgrade() {
                me.scroll_ended();
            }
        });
    }

    fn scroll_ended(self: &Rc<Self>) {
        {
            let mut s = self.state.borrow_mut();
            if !s.scrolling {
                return;
            }
            s.scrolling = false;
            let _ = s.hands.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1));
            s.store.release();
        }
        self.publish(false);
        self.schedule_redraw();
        if self.state.borrow().last_change.is_some() {
            self.schedule_write();
        }
    }
}

impl super::State {
    /// A scroll along a track holds the hand, read by the checks.
    pub fn scrolling(&self) -> bool {
        self.scrolling
    }
}
