//! The watchdog counts a block only while code holds the interface thread: at rest in its run loop, or in the modal
//! loop a save panel runs, the thread answers; held, it is a block. Each check runs on its process's main thread.

#[cfg(target_os = "macos")]
mod run_loop {
    use std::ffi::{CString, c_void};

    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        static kCFRunLoopDefaultMode: *const c_void;
        fn CFRunLoopGetMain() -> *mut c_void;
        fn CFRunLoopAddCommonMode(run_loop: *mut c_void, mode: *const c_void);
        fn CFRunLoopRunInMode(mode: *const c_void, seconds: f64, once: u8) -> i32;
        fn CFStringCreateWithCString(allocator: *const c_void, text: *const std::ffi::c_char, encoding: u32) -> *const c_void;
        fn CFAbsoluteTimeGetCurrent() -> f64;
        fn CFRunLoopTimerCreate(a: *const c_void, at: f64, every: f64, flags: u64, order: isize, call: extern "C" fn(*mut c_void, *mut c_void), c: *mut c_void) -> *mut c_void;
        fn CFRunLoopAddTimer(run_loop: *mut c_void, timer: *mut c_void, mode: *const c_void);
    }

    extern "C" fn never(_: *mut c_void, _: *mut c_void) {}

    /// The main thread at rest in its run loop for `seconds`, in the default mode or in the one AppKit's modal panels
    /// run, made a common mode as AppKit makes it.
    pub fn rest(modal: bool, seconds: f64) {
        // SAFETY: called on the main thread; the mode strings live as long as the process.
        unsafe {
            let mode = if modal {
                let name = CString::new("NSModalPanelRunLoopMode").unwrap();
                let mode = CFStringCreateWithCString(std::ptr::null(), name.as_ptr(), 0x0800_0100);
                CFRunLoopAddCommonMode(CFRunLoopGetMain(), mode);
                // A mode holding nothing returns at once: a panel's mode holds its window's sources, here a far timer.
                let far = CFRunLoopTimerCreate(std::ptr::null(), CFAbsoluteTimeGetCurrent() + 3600.0, 0.0, 0, 0, never, std::ptr::null_mut());
                CFRunLoopAddTimer(CFRunLoopGetMain(), far, mode);
                mode
            } else {
                kCFRunLoopDefaultMode
            };
            let end = std::time::Instant::now() + std::time::Duration::from_secs_f64(seconds);
            while std::time::Instant::now() < end {
                CFRunLoopRunInMode(mode, end.saturating_duration_since(std::time::Instant::now()).as_secs_f64(), 0);
            }
        }
    }
}

pub(crate) fn an_interface_thread_at_rest_or_in_a_modal_loop_answers_and_one_held_is_a_block() {
    #[cfg(not(target_os = "macos"))]
    println!("SKIPPED an_interface_thread_at_rest_or_in_a_modal_loop_answers_and_one_held_is_a_block: its run loop is macOS's");
    #[cfg(target_os = "macos")]
    {
        use oni_app::watchdog::blocks;
        oni_app::watchdog::witness(|| "the check".into());
        oni_app::watchdog::start();
        let before = blocks();
        run_loop::rest(false, 2.0);
        assert_eq!(blocks(), before, "at rest in its run loop, no timer of Slint's running, the interface thread answers");
        run_loop::rest(true, 2.0);
        assert_eq!(blocks(), before, "in the modal loop a save panel runs, it answers");
        // Twin: held by code past the floor, it is a block, ended once the thread rests again.
        std::thread::sleep(oni_app::watchdog::STALL_FLOOR * 3);
        run_loop::rest(false, 0.5);
        assert_eq!(blocks(), before + 1, "held, it is one block");
    }
}
