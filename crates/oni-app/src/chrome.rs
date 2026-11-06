//! `DSWindowChrome`: on macOS the main window's title bar is transparent, untitled and under the
//! content, so the traffic lights sit in the app's own bar; the bar's empty room drags the window.

use slint::winit_030::WinitWindowAccessor;
use slint::winit_030::winit::window::WindowAttributes;
use std::cell::Cell;
use std::time::Instant;

thread_local! {
    /// Set while the main window is built: the attributes hook sees every window and must chrome one.
    static PENDING: Cell<bool> = const { Cell::new(false) };
}

/// The Swift app's icon, which a bare binary has no bundle to carry.
const ICON: &[u8] = include_bytes!("../../oni-ui/ui/icons/app-icon.png");

/// Builds the main window with `build`, so only its winit attributes take the unified bar, and gives
/// the Dock the app's icon.
pub fn unified<T>(build: impl FnOnce() -> T) -> T {
    PENDING.with(|p| p.set(true));
    let built = build();
    PENDING.with(|p| p.set(false));
    platform::dock_icon(ICON);
    built
}

/// The backend's attributes hook: the window being built under `unified` loses its title bar's
/// fill and title, and its content runs under it.
pub fn attributes(attributes: WindowAttributes) -> WindowAttributes {
    if !PENDING.with(|p| p.replace(false)) {
        return attributes;
    }
    platform::unify(attributes)
}

/// Centres the traffic lights in a bar `bar` points high and answers the room they take from the
/// window's left edge; zero when they are hidden (full screen) or the system draws its own bar.
pub fn traffic_lights(window: &slint::Window, bar: f32) -> f32 {
    window.with_winit_window(|w| if w.fullscreen().is_some() { 0.0 } else { platform::traffic_lights(w, f64::from(bar)) }).unwrap_or(0.0)
}

thread_local! {
    /// The last press on the bar's room, which the drag swallows before Slint could pair two.
    static LAST_PRESS: Cell<Option<Instant>> = const { Cell::new(None) };
}

/// A press on the bar's empty room moves the window as a title bar does, and a second within the
/// double-click interval does what the person set for one; elsewhere the system's bar stays.
pub fn room_pressed(window: &slint::Window) {
    if !cfg!(target_os = "macos") {
        return;
    }
    let now = Instant::now();
    let second = LAST_PRESS.with(|l| l.replace(Some(now))).is_some_and(|t| now - t <= crate::system::double_click());
    window.with_winit_window(|w| {
        if !second {
            let _ = w.drag_window();
            return;
        }
        LAST_PRESS.with(|l| l.set(None));
        match platform::double_click_action() {
            Action::Minimise => w.set_minimized(true),
            Action::Zoom => w.set_maximized(!w.is_maximized()),
            Action::Nothing => {}
        }
    });
}

enum Action {
    Zoom,
    Minimise,
    Nothing,
}

#[cfg(target_os = "macos")]
mod platform {
    use super::Action;
    use objc2_app_kit::{NSView, NSWindowButton};
    use objc2_foundation::{NSPoint, NSString, NSUserDefaults};
    use slint::winit_030::winit::platform::macos::WindowAttributesExtMacOS;
    use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use slint::winit_030::winit::window::{Window, WindowAttributes};

    pub fn unify(attributes: WindowAttributes) -> WindowAttributes {
        attributes.with_titlebar_transparent(true).with_fullsize_content_view(true).with_title_hidden(true)
    }

    pub fn dock_icon(png: &[u8]) {
        let Some(mtm) = objc2::MainThreadMarker::new() else { return };
        let data = objc2_foundation::NSData::with_bytes(png);
        let Some(image) = objc2_app_kit::NSImage::initWithData(objc2::AllocAnyThread::alloc(), &data) else { return };
        // SAFETY: on the main thread, with an image AppKit retains for as long as it shows it.
        unsafe { objc2_app_kit::NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&image)) };
    }

    /// AppKit lays the lights again on every resize, so each call lays them back; the room is the zoom
    /// key's right edge plus the close key's margin, as far from the lights as they sit from the edge.
    pub fn traffic_lights(window: &Window, bar: f64) -> f32 {
        let Ok(handle) = window.window_handle() else { return 0.0 };
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else { return 0.0 };
        // SAFETY: winit hands out its live `NSView`, read here on the main thread that owns it.
        let view: &NSView = unsafe { handle.ns_view.cast().as_ref() };
        let Some(ns_window) = view.window() else { return 0.0 };
        let (Some(close), Some(zoom)) = (ns_window.standardWindowButton(NSWindowButton::CloseButton), ns_window.standardWindowButton(NSWindowButton::ZoomButton)) else {
            return 0.0;
        };
        if zoom.isHidden() {
            return 0.0;
        }
        // The keys' row is the title bar's container: as high as the bar, its keys centred in it.
        if let Some(row) = unsafe { close.superview().and_then(|v| v.superview()) } {
            let mut frame = row.frame();
            frame.size.height = bar;
            frame.origin.y = ns_window.frame().size.height - bar;
            row.setFrame(frame);
        }
        for key in [NSWindowButton::CloseButton, NSWindowButton::MiniaturizeButton, NSWindowButton::ZoomButton] {
            if let Some(key) = ns_window.standardWindowButton(key) {
                let at = key.frame();
                key.setFrameOrigin(NSPoint::new(at.origin.x, ((bar - at.size.height) / 2.0).round()));
            }
        }
        let (close, zoom) = (close.frame(), zoom.frame());
        (zoom.origin.x + zoom.size.width + close.origin.x) as f32
    }

    /// `AppleActionOnDoubleClick`: Maximize (the default), Minimize or None.
    pub fn double_click_action() -> Action {
        let defaults = NSUserDefaults::standardUserDefaults();
        let set = defaults.stringForKey(&NSString::from_str("AppleActionOnDoubleClick")).map(|s| s.to_string());
        match set.as_deref() {
            Some("Minimize") => Action::Minimise,
            Some("None") => Action::Nothing,
            _ => Action::Zoom,
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use super::Action;
    use slint::winit_030::winit::window::{Window, WindowAttributes};

    pub fn unify(attributes: WindowAttributes) -> WindowAttributes {
        attributes
    }

    /// Windows takes the icon from the window's own `icon`.
    pub fn dock_icon(_: &[u8]) {}

    pub fn traffic_lights(_: &Window, _: f64) -> f32 {
        0.0
    }

    pub fn double_click_action() -> Action {
        Action::Nothing
    }
}
