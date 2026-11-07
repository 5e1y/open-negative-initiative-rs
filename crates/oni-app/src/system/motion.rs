//! The system's Reduce Motion: what moves only to answer a gesture stands still under it.

/// `NSWorkspace.accessibilityDisplayShouldReduceMotion`, called by name: its typed binding sits in an
/// AppKit feature the rest of the app never needs.
#[cfg(target_os = "macos")]
pub fn reduce_motion() -> bool {
    let workspace = objc2_app_kit::NSWorkspace::sharedWorkspace();
    // SAFETY: a BOOL property NSWorkspace has answered since macOS 10.12, read with no argument.
    unsafe { objc2::msg_send![&*workspace, accessibilityDisplayShouldReduceMotion] }
}

/// Windows' "Show animations in Windows" (`SPI_GETCLIENTAREAANIMATION`), off.
#[cfg(target_os = "windows")]
pub fn reduce_motion() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SPI_GETCLIENTAREAANIMATION, SystemParametersInfoW};
    let mut animated: windows_sys::core::BOOL = 1;
    // SAFETY: a read of one system setting into the BOOL it is given room for.
    let read = unsafe { SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, (&raw mut animated).cast(), 0) };
    read != 0 && animated == 0
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn reduce_motion() -> bool {
    false
}
