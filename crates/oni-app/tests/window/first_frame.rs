//! A window's first drawable on AppKit and Metal themselves, no window ever on screen: it owes nothing to
//! the occlusion state, or a window whose state never turns visible shows nothing, with no error.

pub(crate) fn a_first_drawable_owes_nothing_to_the_occlusion_state() {
    #[cfg(target_os = "macos")]
    mac::check();
}

#[cfg(target_os = "macos")]
mod mac {
    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject, Imp, Sel};
    use objc2::{MainThreadMarker, MainThreadOnly, msg_send, sel};
    use objc2_app_kit::{NSApplication, NSWindow, NSWindowOcclusionState};
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    use slint::winit_030::winit::raw_window_handle::{AppKitDisplayHandle, AppKitWindowHandle, RawDisplayHandle, RawWindowHandle};
    use std::ptr::NonNull;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use wgpu::CurrentSurfaceTexture as Frame;

    /// `NSWindowOcclusionStateVisible`.
    const VISIBLE: usize = 1 << 1;
    /// What every window answers once `lie` has run.
    static SAID: AtomicUsize = AtomicUsize::new(0);

    extern "C-unwind" fn said(_: *mut AnyObject, _: Sel) -> usize {
        SAID.load(Ordering::Relaxed)
    }

    /// AppKit's occlusion state answers from `SAID`, so a window off screen can claim to have been seen.
    fn lie() {
        let method = AnyClass::get(c"NSWindow").and_then(|c| c.instance_method(sel!(occlusionState))).expect("NSWindow has an occlusion state");
        // SAFETY: `said` has the signature of the method it replaces, which returns an `NSUInteger`.
        unsafe {
            let imp: Imp = std::mem::transmute(said as extern "C-unwind" fn(*mut AnyObject, Sel) -> usize);
            method.set_implementation(imp);
        }
    }

    pub fn check() {
        let Ok(gpu) = oni_app::render::Device::open() else {
            return println!("SKIPPED no GPU");
        };
        let mtm = MainThreadMarker::new().expect("a window check runs on the main thread");
        let _ = NSApplication::sharedApplication(mtm);
        let rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(64.0, 64.0));
        // SAFETY: `NSWindowStyleMaskBorderless` and `NSBackingStoreBuffered`, on the main thread.
        let window: Retained<NSWindow> = unsafe { msg_send![NSWindow::alloc(mtm), initWithContentRect: rect, styleMask: 0usize, backing: 2usize, defer: false] };
        // SAFETY: the window is held by this function for as long as it uses it.
        unsafe { window.setReleasedWhenClosed(false) };
        let view = window.contentView().expect("a window has a content view");
        let target = wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: Some(RawDisplayHandle::AppKit(AppKitDisplayHandle::new())),
            raw_window_handle: RawWindowHandle::AppKit(AppKitWindowHandle::new(NonNull::from(&*view).cast())),
        };
        // SAFETY: the view outlives the surface, both held to the end of this function.
        let surface = unsafe { gpu.instance.create_surface_unsafe(target) }.expect("a surface on the view");
        let mut config = surface.get_default_config(&gpu.adapter, 64, 64).expect("the adapter presents to the view");
        config.present_mode = wgpu::PresentMode::AutoNoVsync;
        config.desired_maximum_frame_latency = 2;
        surface.configure(&gpu.device, &config);

        // The system's own answer: never ordered in, the window is not visible, and still gets its frame.
        assert!(!window.occlusionState().contains(NSWindowOcclusionState::Visible), "a window never shown is not visible");
        assert!(matches!(surface.get_current_texture(), Frame::Success(_)), "a window never seen visible acquires its first frame");

        // Twin: seen visible once, an occluded window is refused, as a drawable it presented would stick.
        lie();
        SAID.store(VISIBLE, Ordering::Relaxed);
        assert!(matches!(surface.get_current_texture(), Frame::Success(_)));
        SAID.store(0, Ordering::Relaxed);
        assert!(matches!(surface.get_current_texture(), Frame::Occluded), "a window seen visible and now covered is refused");
        SAID.store(VISIBLE, Ordering::Relaxed);
        assert!(matches!(surface.get_current_texture(), Frame::Success(_)), "visible again, it acquires again");
    }
}
