//! The service's Retry-After honoured: no call until it is over, and the latest search asked again once
//! then, so a word typed meanwhile is never lost.

use super::*;

impl KitState {
    /// What is left of the wait the service asked, none once over.
    pub(super) fn held(&self) -> Option<Duration> {
        self.not_before.and_then(|at| at.checked_duration_since(Instant::now())).filter(|left| !left.is_zero())
    }
}

impl Ctl {
    /// The latest search asked again when the wait is over, however many words came meanwhile.
    pub(super) fn kit_ask_after_wait(self: &Rc<Self>) {
        let Some(left) = self.state.borrow().org.kit.held() else { return };
        let me = Rc::downgrade(self);
        self.state.borrow().org.kit.after_wait.start(slint::TimerMode::SingleShot, left, move || {
            if let Some(me) = me.upgrade() {
                me.kit_ask();
            }
        });
    }
}
