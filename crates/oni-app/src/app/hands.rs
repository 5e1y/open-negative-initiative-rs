//! A hand on a control, counted while it lives, and a slider's drag ended by whichever of its two owners
//! ends first: the frame it edits, left, or the left button, lifted.

use super::*;

/// A hand on a control, counted while it lives: the thumbnails' decodes wait on the count.
pub(crate) struct HandDown(Arc<AtomicUsize>);

impl HandDown {
    pub(crate) fn on(hands: &Arc<AtomicUsize>) -> Self {
        hands.fetch_add(1, Ordering::Relaxed);
        Self(hands.clone())
    }
}

impl Drop for HandDown {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

/// A drag under way: where the knob stood at the press, the value under the hand, the hand counted.
pub(in crate::app) struct Drag {
    pub(in crate::app) from: f32,
    pub(in crate::app) live: Option<f32>,
    _down: HandDown,
}

impl Drag {
    pub(in crate::app) fn at(from: f32, hands: &Arc<AtomicUsize>) -> Self {
        Self { from, live: None, _down: HandDown::on(hands) }
    }
}

impl Slide {
    /// The value under the hand, drawn ahead of the store.
    pub(in crate::app) fn live(&self) -> Option<f32> {
        self.drag.as_ref()?.live
    }
}

impl Ctl {
    /// The hand lifts off `f`: its last value applied and recorded as one step on the frame it was on.
    /// Idempotent: a release with no drag records only what the store still holds, a rest's step.
    pub(in crate::app) fn let_go(self: &Rc<Self>, f: Field) {
        let i = f.index() as usize;
        let live = {
            let mut s = self.state.borrow_mut();
            let slide = &mut s.slides[i];
            slide.hand.released();
            slide.drag.take().and_then(|d| d.live)
        };
        self.trailing[i].stop();
        if let Some(live) = live {
            let mut s = self.state.borrow_mut();
            let command = store::moved(&*s.store, f, live);
            s.store.apply(command);
        }
        let released = Instant::now();
        self.state.borrow_mut().store.release();
        // The step's record and any roll it reaches, read and written on this thread.
        if released.elapsed() > Duration::from_millis(5) {
            timing::line(&format!("a release recorded its step in {:.1} ms", released.elapsed().as_secs_f64() * 1000.0));
        }
        self.publish(false);
        self.schedule_redraw();
        if self.state.borrow().last_change.is_some() {
            self.schedule_write();
        }
        // The readout leaves the hand's value for the store's, which a clamp may have moved.
        if let Some(w) = self.window() {
            let v = self.state.borrow().store.get(f);
            set_readout(&w.global::<StoreGlobal>(), f, self.readout_at(f, v).into());
        }
    }

    /// Every hand on a slider, a plot or a track lifted: the frame is left, or the button rose wherever
    /// the item went, as Slint tells a grabbing item destroyed under the hand nothing.
    pub(in crate::app) fn let_go_all(self: &Rc<Self>) {
        let held: Vec<Field> = Field::ALL.into_iter().filter(|f| self.state.borrow().slides[f.index() as usize].drag.is_some()).collect();
        for f in held {
            self.let_go(f);
        }
        crate::editors::let_go(self);
        self.scroll_ended();
    }

    /// The window's left button rose, whatever became of the item it pressed.
    pub fn lifted(self: &Rc<Self>) {
        self.let_go_all();
    }
}

impl State {
    /// The hands on controls holding the thumbnails' decodes back, read by the checks.
    pub fn hands(&self) -> usize {
        self.hands.load(Ordering::Relaxed)
    }

    /// A plot's hand counted while the editors' gestures hold one.
    pub(crate) fn held_plot(&mut self, holding: bool) {
        if !holding {
            self.plot_down = None;
        } else if self.plot_down.is_none() {
            self.plot_down = Some(HandDown::on(&self.hands));
        }
    }

    /// A slider is under the hand, read by the checks.
    pub fn dragging(&self) -> bool {
        self.slides.iter().any(|s| s.drag.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hand_counts_while_it_lives() {
        let hands = Arc::new(AtomicUsize::new(0));
        let (one, two) = (HandDown::on(&hands), HandDown::on(&hands));
        assert_eq!(hands.load(Ordering::Relaxed), 2);
        drop(one);
        assert_eq!(hands.load(Ordering::Relaxed), 1);
        // Twin: a token never dropped leaves its hand counted, so the drop is what lowers it.
        std::mem::forget(two);
        assert_eq!(hands.load(Ordering::Relaxed), 1);
    }
}
