//! LibRaw's C interface as `csrc` exposes it, and the session that frees its buffers on every exit path.

use std::os::raw::{c_char, c_float, c_int, c_ushort, c_void};
use std::sync::Mutex;

#[repr(C)]
pub(crate) struct Decoded {
    pub(crate) width: c_int,
    pub(crate) height: c_int,
    pub(crate) colors: c_int,
    pub(crate) dng: c_int,
    pub(crate) cam_xyz: [[c_float; 3]; 4],
    pub(crate) baseline_exposure: c_float,
    pub(crate) pixels: *const c_ushort,
    pub(crate) image: *mut c_void,
    pub(crate) raw: *mut c_void,
    pub(crate) half: c_int,
    pub(crate) inset: [c_int; 4],
    pub(crate) flip: c_int,
    pub(crate) stream: *mut c_void,
    pub(crate) cut: c_int,
    pub(crate) cam_mul: [c_float; 4],
}

unsafe extern "C" {
    pub(crate) fn oni_open(path: *const c_char, stream: *mut c_void, exposure_gain: c_float, min_side: c_int, out: *mut Decoded) -> c_int;
    pub(crate) fn oni_unpack(d: *mut Decoded) -> c_int;
    pub(crate) fn oni_inset(d: *const Decoded, out: *mut c_int);
    pub(crate) fn oni_process(d: *mut Decoded, mul: *const c_float) -> c_int;
    pub(crate) fn oni_release(d: *mut Decoded);
    pub(crate) fn oni_mosaic(d: *const Decoded, counts: *mut *const c_ushort, out: *mut c_int) -> c_int;
}

/// Frees LibRaw's buffers on every exit path.
pub(crate) struct Session(pub(crate) Decoded);

impl Drop for Session {
    fn drop(&mut self) {
        unsafe { oni_release(&mut self.0) };
    }
}

impl Session {
    pub(crate) fn unpack(&mut self) -> i32 {
        let _held = (self.0.dng != 0).then(|| DNG_UNPACK.lock().unwrap_or_else(|e| e.into_inner()));
        unsafe { oni_unpack(&mut self.0) }
    }
}

/// LibRaw's DNG unpacker fills a static cosine table on first use, unguarded (`ljpeg_idct`): DNG
/// unpacks take this lock, every other stage of every decode runs concurrently.
pub(crate) static DNG_UNPACK: Mutex<()> = Mutex::new(());
