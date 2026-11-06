//! The app's icon in the Dock, which a bare binary has no bundle to carry. The window keeps the
//! system's own title bar, its traffic lights in it (owner's call).

/// The Swift app's icon.
const ICON: &[u8] = include_bytes!("../../oni-ui/ui/icons/app-icon.png");

/// Gives the Dock the app's icon.
pub fn dock_icon() {
    platform::dock_icon(ICON);
}

#[cfg(target_os = "macos")]
mod platform {
    pub fn dock_icon(png: &[u8]) {
        let Some(mtm) = objc2::MainThreadMarker::new() else { return };
        let data = objc2_foundation::NSData::with_bytes(png);
        let Some(image) = objc2_app_kit::NSImage::initWithData(objc2::AllocAnyThread::alloc(), &data) else { return };
        // SAFETY: on the main thread, with an image AppKit retains for as long as it shows it.
        unsafe { objc2_app_kit::NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&image)) };
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    /// Windows takes the icon from the window's own `icon`.
    pub fn dock_icon(_: &[u8]) {}
}
