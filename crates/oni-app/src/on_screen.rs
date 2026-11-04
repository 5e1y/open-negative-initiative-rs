//! What the system shows of the main window, written to the log: a window that stays away or keeps its
//! old size raises no error, and a report from the machine has to say which.

use crate::MainWindow;
use slint::winit_030::WinitWindowAccessor;
use slint::winit_030::winit::monitor::MonitorHandle;
use slint::winit_030::winit::window::Window;
use slint::{ComponentHandle, Timer, TimerMode, Weak};
use std::time::Duration;

/// How long after showing the window its state is written, first frame or not.
const AFTER_SHOWING: Duration = Duration::from_secs(3);
/// The calm after a resize or a change of screen before its state is written: one line a gesture.
const SETTLE: Duration = Duration::from_millis(500);

thread_local! {
    static SETTLING: Timer = Timer::default();
}

/// The screens now, and the window's state a moment after showing it.
pub fn shown(window: Weak<MainWindow>) {
    if let Some(list) = window.upgrade().and_then(|w| w.window().with_winit_window(screens)) {
        crate::timing::line(&format!("screens: {list}"));
    }
    Timer::single_shot(AFTER_SHOWING, move || write("window after showing", &window));
}

/// A resize or a change of screen of a window shown: the state is written once it has settled.
pub fn settling(window: Weak<MainWindow>) {
    if window.upgrade().is_some_and(|w| w.window().is_visible()) {
        SETTLING.with(|t| t.start(TimerMode::SingleShot, SETTLE, move || write("window settled", &window)));
    }
}

fn write(what: &str, window: &Weak<MainWindow>) {
    if let Some(state) = window.upgrade().and_then(|w| w.window().with_winit_window(platform::state)) {
        crate::timing::line(&format!("{what}: {state}, {} frames drawn", crate::hz::frames()));
    }
}

/// Each screen's name, pixels, scale and rate, the window's marked.
fn screens(window: &Window) -> String {
    let current = window.current_monitor();
    let one = |m: MonitorHandle| {
        let (size, hz) = (m.size(), m.refresh_rate_millihertz().unwrap_or(0) / 1000);
        let here = if current.as_ref() == Some(&m) { " (the window's)" } else { "" };
        format!("{} {}x{} ×{} {hz} Hz{here}", m.name().unwrap_or_default(), size.width, size.height, m.scale_factor())
    };
    window.available_monitors().map(one).collect::<Vec<_>>().join(", ")
}

#[cfg(target_os = "macos")]
mod platform {
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::{NSView, NSWindowOcclusionState};
    use objc2_foundation::{NSArray, NSRect, NSSize};
    use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use slint::winit_030::winit::window::Window;

    /// The window's alpha and occlusion as AppKit answers, its frame, its view and the Metal layer's
    /// bounds and drawable: a layer that differs from its view is what shows.
    pub fn state(window: &Window) -> String {
        let Ok(handle) = window.window_handle() else { return "no handle".into() };
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else { return "no view".into() };
        // SAFETY: winit hands out its live `NSView`, read here on the main thread that owns it.
        let view: &NSView = unsafe { handle.ns_view.cast().as_ref() };
        let Some(ns_window) = view.window() else { return "no window".into() };
        let (frame, bounds) = (ns_window.frame(), view.bounds());
        let visible = ns_window.occlusionState().contains(NSWindowOcclusionState::Visible);
        let mut state = format!(
            "alpha {:.2}, ordered {}, occlusion says {}, frame {:.0}x{:.0} at {:.0},{:.0}, view {:.0}x{:.0}",
            ns_window.alphaValue(),
            if ns_window.isVisible() { "in" } else { "out" },
            if visible { "visible" } else { "not visible" },
            frame.size.width,
            frame.size.height,
            frame.origin.x,
            frame.origin.y,
            bounds.size.width,
            bounds.size.height,
        );
        // SAFETY: selectors of `NSView` and `CALayer`; the sublayer is the one wgpu made, a `CAMetalLayer`.
        unsafe {
            let root: Option<Retained<AnyObject>> = msg_send![view, layer];
            let subs: Option<Retained<NSArray<AnyObject>>> = root.as_ref().and_then(|r| msg_send![&**r, sublayers]);
            if let Some(layer) = subs.and_then(|s| s.firstObject()) {
                let (bounds, drawable): (NSRect, NSSize) = (msg_send![&*layer, bounds], msg_send![&*layer, drawableSize]);
                state += &format!(", layer {:.0}x{:.0}, drawable {:.0}x{:.0}", bounds.size.width, bounds.size.height, drawable.width, drawable.height);
            }
        }
        state
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use slint::winit_030::winit::window::Window;

    pub fn state(window: &Window) -> String {
        let (size, at) = (window.inner_size(), window.outer_position().unwrap_or_default());
        format!("inner {}x{} at {},{}, visible {:?}", size.width, size.height, at.x, at.y, window.is_visible())
    }
}
