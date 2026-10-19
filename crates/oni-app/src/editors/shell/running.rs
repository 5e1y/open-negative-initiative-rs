//! A gesture run against the store, and the settle or redraw it asks for after.

use super::*;



/// Runs a gesture on the store, keeping the thumbnails' decodes back while a press holds.
pub(super) fn run(ctl: &Rc<Ctl>, f: impl FnOnce(&mut Gestures, &mut dyn crate::store::Store) -> Then) -> Then {
    let mut s = ctl.state.borrow_mut();
    let mut g = ctl.editors.gestures.borrow_mut();
    let before = g.holding();
    let then = f(&mut g, s.store.as_mut());
    match (before, g.holding()) {
        (false, true) => {
            s.hands.fetch_add(1, Ordering::Relaxed);
        }
        (true, false) => {
            let _ = s.hands.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1));
        }
        _ => {}
    }
    let wait = s.store.settle_if_due();
    drop((g, s));
    if let Some(wait) = wait {
        settle_after(ctl, wait);
    }
    then
}

/// Restarts the settle clock, as each change restarts Swift's recording task.
pub(super) fn settle_after(ctl: &Rc<Ctl>, wait: std::time::Duration) {
    let me = Rc::downgrade(ctl);
    ctl.editors.settling.start(TimerMode::SingleShot, wait, move || {
        let Some(me) = me.upgrade() else { return };
        let wait = me.state.borrow_mut().store.settle_if_due();
        match wait {
            Some(wait) => settle_after(&me, wait),
            None => me.publish(false),
        }
    });
}

pub(super) fn after(ctl: &Rc<Ctl>, then: Then) {
    match then {
        Then::Nothing => {}
        Then::Draw => draw(ctl, false),
        Then::Publish => ctl.publish(false),
        Then::Trail(delay) => {
            draw(ctl, false);
            let me = Rc::downgrade(ctl);
            ctl.editors.trailing.start(TimerMode::SingleShot, delay, move || {
                let Some(me) = me.upgrade() else { return };
                let then = run(&me, |g, store| g.trailed(store, Instant::now()));
                after(&me, then);
            });
        }
        Then::Released => {
            ctl.editors.trailing.stop();
            ctl.publish(false);
        }
    }
}
