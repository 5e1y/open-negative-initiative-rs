//! The bench's window filling its screen.

use slint::ComponentHandle;

/// The window filling its screen, asked outside Slint's timers: AppKit's zoom animates in a nested run loop whose
/// display link runs Slint's timers, which refuse to run inside one of their own ("Recursion in timer code").
pub(crate) fn maximise(w: &crate::MainWindow) {
    let weak = w.as_weak();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(w) = weak.upgrade() {
            w.window().set_maximized(true);
        }
    });
}
