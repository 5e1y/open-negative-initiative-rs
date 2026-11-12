//! A file's full decode size read from its metadata alone, upright as the decode hands it out: what a
//! fraction of its scan is of, asked without decoding a pixel.

use crate::source::{Route, Source};
use crate::{Branch, linear, plain, route};
use std::os::raw::{c_char, c_int, c_void};
use std::path::Path;

unsafe extern "C" {
    fn oni_size(path: *const c_char, stream: *mut c_void, out: *mut c_int) -> c_int;
}

/// The width and height the whole decode of `path` has; `None` for a file that does not read.
pub fn full_size(path: &Path) -> Option<(usize, usize)> {
    match route(path) {
        Branch::Mosaic => mosaic(path),
        Branch::LinearRgb => crate::container::pixel_size(path),
        Branch::Plain => plain::size(path),
    }
}

/// LibRaw's processed sides, cut to the maker's crop where it lies inside them, as the decode cuts it.
fn mosaic(path: &Path) -> Option<(usize, usize)> {
    let mut source = Source::open(path, Route::PLATFORM).ok()?;
    let mut out = [0 as c_int; 7];
    if unsafe { oni_size(source.name(), source.take(), out.as_mut_ptr()) } != 0 {
        return None;
    }
    let [w, h, cx, cy, cw, ch] = [out[0], out[1], out[2], out[3], out[4], out[5]].map(|v| v.max(0) as usize);
    let kept = if cw > 0 && ch > 0 && cx + cw <= w && cy + ch <= h { (cw, ch) } else { (w, h) };
    (kept.0 > 0 && kept.1 > 0).then(|| linear::upright(out[6], kept))
}
