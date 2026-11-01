//! A frame's tile as Prepare the Roll draws it: the head of the chain on the measuring copy, reduced.

use crate::reader::{Copy, Reader};
use oni_settings::PipelineSettings;

/// The side a tile is reduced to: a roll of 36 frames six to a row, 2:1 cells at most and a 4 %
/// gap, held in 2^24 pixels, Core Image's single pass.
pub fn roll_tile() -> f64 {
    const PIXELS: f64 = 16_777_216.0;
    let (columns, rows, gap, widest) = (6.0, 6.0, 0.04, 2.0);
    (PIXELS / ((columns + gap * (columns + 1.0)) * (rows / widest + gap * (rows + 1.0)))).sqrt()
}

/// A frame's pixels, rows top first, premultiplied.
#[derive(Debug, Clone, PartialEq)]
pub struct Picture {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<[f32; 4]>,
}

/// `ContactSheet.tile`: the head of the chain on a frame, `Pipeline.framed` alone, reduced to `side`.
pub fn tile(r: &mut Reader, copy: Copy, s: &PipelineSettings, side: f64) -> Option<Picture> {
    let e = copy.source.extent();
    let full = copy.full_width.unwrap_or(f64::from(e.w));
    let scale = if full > 0.0 { f64::from(e.w) / full } else { 1.0 };
    let mut framed = r.framed(copy.source, s, scale)?;
    let f = framed.extent();
    let longest = f64::from(f.w.max(f.h));
    if longest > side {
        framed = framed.lanczos(side / longest);
    }
    let f = framed.extent();
    if f.w == 0 || f.h == 0 {
        return None;
    }
    let pixels = r.graph.render(&framed, f, None).ok()?;
    Some(Picture { width: f.w as usize, height: f.h as usize, pixels })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_roll_s_tile_holds_thirty_six_frames_in_two_to_the_twenty_four_pixels() {
        let side = roll_tile();
        assert!((side - 902.5).abs() < 0.5, "{side}");
        // Twin: a pixel more a side and the roll outgrows the pass.
        let held = |s: f64| (6.0 * s + 0.04 * 7.0 * s) * (3.0 * s + 0.04 * 7.0 * s);
        assert!(held(side) <= 16_777_216.0 * 1.000_001 && held(side + 1.0) > 16_777_216.0);
    }
}
