//! Slint's clock on the headless window: moved ahead of the real one, or held still and moved by hand.

use super::Rig;
use std::time::Duration;

impl Rig {
    /// Slint's clock moved `by` while the real one stands, then the timers it reached run.
    pub fn advance(&self, by: Duration) {
        match self.frozen.get() {
            Some(at) => self.frozen.set(Some(at + by)),
            None => self.ahead.set(self.ahead.get() + by),
        }
        slint::platform::update_timers_and_animations();
    }

    /// Slint's clock held where it stands, `advance` alone moving it, until `thaw`.
    pub fn freeze(&self) {
        if self.frozen.get().is_none() {
            self.frozen.set(Some(self.start.elapsed() + self.ahead.get()));
        }
        slint::platform::update_timers_and_animations();
    }

    /// Slint's clock back on the real one, never behind where it was held.
    pub fn thaw(&self) {
        if let Some(at) = self.frozen.take() {
            self.ahead.set(at.saturating_sub(self.start.elapsed()).max(self.ahead.get()));
        }
        slint::platform::update_timers_and_animations();
    }
}
