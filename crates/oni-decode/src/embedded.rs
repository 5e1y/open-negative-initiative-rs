//! The preview a RAW carries, read without demosaicing: what Swift's ImageIO hands out for a frame
//! not yet converted, in about a hundredth of a decode's time.

use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_uint, c_void};
use std::path::Path;

#[repr(C)]
struct Thumb {
    format: c_int,
    width: c_int,
    height: c_int,
    colors: c_int,
    bits: c_int,
    flip: c_int,
    size: c_uint,
    data: *const u8,
    image: *mut c_void,
}

unsafe extern "C" {
    fn oni_thumbnail(path: *const c_char, out: *mut Thumb) -> c_int;
    fn oni_thumb_release(t: *mut Thumb);
}

/// LibRaw's `LIBRAW_IMAGE_JPEG` and `LIBRAW_IMAGE_BITMAP`.
const JPEG: c_int = 1;
const BITMAP: c_int = 2;

/// The preview as the file holds it: a JPEG's bytes, or 8-bit RGB rows.
pub enum Preview {
    Jpeg(Vec<u8>),
    Rgb { width: usize, height: usize, pixels: Vec<u8> },
}

/// The preview, unturned, and LibRaw's `flip` to turn it by.
pub struct Embedded {
    pub preview: Preview,
    pub flip: i32,
}

/// The largest preview LibRaw finds in the file; `None` for a file that carries none it reads.
pub fn embedded(path: &Path) -> Option<Embedded> {
    let c_path = CString::new(path.to_string_lossy().as_bytes()).ok()?;
    let mut t: Thumb = unsafe { std::mem::zeroed() };
    let rc = {
        let _held = crate::DNG_UNPACK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { oni_thumbnail(c_path.as_ptr(), &mut t) }
    };
    if rc != 0 {
        return None;
    }
    let bytes = if t.data.is_null() { &[][..] } else { unsafe { std::slice::from_raw_parts(t.data, t.size as usize) } };
    let preview = match (t.format, t.colors, t.bits) {
        (JPEG, _, _) => Some(Preview::Jpeg(bytes.to_vec())),
        (BITMAP, 3, 8) => Some(Preview::Rgb { width: t.width as usize, height: t.height as usize, pixels: bytes.to_vec() }),
        _ => None,
    };
    let flip = t.flip;
    unsafe { oni_thumb_release(&mut t) };
    preview.map(|preview| Embedded { preview, flip })
}

/// RGB rows `w` by `h` turned by LibRaw's `flip`, as a decode is: bit 4 swaps rows and columns,
/// then bit 2 mirrors the rows and bit 1 the columns.
pub fn turned(pixels: &[u8], (w, h): (usize, usize), flip: i32) -> (Vec<u8>, usize, usize) {
    let (ow, oh) = crate::linear::upright(flip, (w, h));
    let mut out = vec![0u8; ow * oh * 3];
    for row in 0..oh {
        for col in 0..ow {
            let (mut sr, mut sc) = if flip & 4 != 0 { (col, row) } else { (row, col) };
            if flip & 2 != 0 {
                sr = h - 1 - sr;
            }
            if flip & 1 != 0 {
                sc = w - 1 - sc;
            }
            let (s, d) = ((sr * w + sc) * 3, (row * ow + col) * 3);
            out[d..d + 3].copy_from_slice(&pixels[s..s + 3]);
        }
    }
    (out, ow, oh)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 3 by 2 image whose red names each pixel by its place, turned by each flip LibRaw writes.
    #[test]
    fn each_flip_turns_the_preview_as_a_decode_is_turned() {
        let pixels: Vec<u8> = (0..6u8).flat_map(|i| [i, 0, 0]).collect();
        let reds = |flip| {
            let (out, w, h) = turned(&pixels, (3, 2), flip);
            ((w, h), out.chunks(3).map(|p| p[0]).collect::<Vec<_>>())
        };
        assert_eq!(reds(0), ((3, 2), vec![0, 1, 2, 3, 4, 5]));
        assert_eq!(reds(3), ((3, 2), vec![5, 4, 3, 2, 1, 0]), "half turn");
        assert_eq!(reds(6), ((2, 3), vec![3, 0, 4, 1, 5, 2]), "a quarter turn right");
        assert_eq!(reds(5), ((2, 3), vec![2, 5, 1, 4, 0, 3]), "a quarter turn left");
        // Twin: the two quarter turns are not each other.
        assert_ne!(reds(5), reds(6));
    }
}
