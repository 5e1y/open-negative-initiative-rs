//! The real rate of the picture: each second, the window's frames drawn and those in which the
//! canvas showed something new (a render landed, or the picture moved), 0 when nothing changes.
//! `ONI_HZ=1` shows it by the zoom; the log keeps every second something changed.

use crate::{Canvas, MainWindow};
use slint::{ComponentHandle, Timer, TimerMode};
use std::cell::Cell;
use std::time::Duration;

thread_local! {
    /// The canvas changed since the last frame drawn; frames drawn, and those with a change, this second.
    static METER: (Cell<bool>, Cell<u32>, Cell<u32>) = const { (Cell::new(false), Cell::new(0), Cell::new(0)) };
}

/// The canvas shows something new from the next frame drawn.
pub fn changed() {
    METER.with(|m| m.0.set(true));
}

/// A frame drawn, counted as a canvas frame when the canvas changed since the one before.
fn drawn() {
    METER.with(|m| {
        m.1.set(m.1.get() + 1);
        if m.0.replace(false) {
            m.2.set(m.2.get() + 1);
        }
    });
}

/// Counts every frame `window` draws, and each second publishes and logs the two rates.
pub fn watch(window: &MainWindow) -> Option<Timer> {
    if window.window().set_rendering_notifier(|state, _| {
        if matches!(state, slint::RenderingState::AfterRendering) {
            drawn();
        }
    }).is_err() {
        return None;
    }
    let shown = std::env::var_os("ONI_HZ").is_some_and(|v| v == "1");
    let weak = window.as_weak();
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_secs(1), move || {
        let (frames, canvas) = METER.with(|m| (m.1.replace(0), m.2.replace(0)));
        if canvas > 0 {
            crate::timing::line(&format!("hz: canvas {canvas}, window {frames}"));
        }
        if let Some(w) = weak.upgrade().filter(|_| shown) {
            w.global::<Canvas>().set_hz(format!("{canvas} Hz · {frames}").into());
        }
    });
    Some(timer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_counts_for_the_canvas_only_when_the_canvas_changed_before_it() {
        METER.with(|m| (m.0.set(false), m.1.set(0), m.2.set(0)));
        drawn();
        changed();
        drawn();
        drawn();
        changed();
        changed();
        drawn();
        // Four frames drawn, two of them after a change; two changes before one frame count once.
        assert_eq!(METER.with(|m| (m.1.get(), m.2.get())), (4, 2));
        // Twin: a meter counting every frame for the canvas would read 4, the window's rate.
        assert_ne!(METER.with(|m| m.2.get()), METER.with(|m| m.1.get()));
    }
}
