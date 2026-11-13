//! The before and after joined in their real frames: a hard edge travels left to right, the after's
//! pixels left of it and the before's right of it, so every pixel shown is one version's own.

use crate::film::FPS;
use crate::photo::Rgba;
use crate::track::ease;

/// `before` with `after` laid left of an edge at `progress` of the width: 0 is the before whole, 1
/// the after whole. The two frames are shot of the same region and so have one size.
pub fn wipe(before: &Rgba, after: &Rgba, progress: f32) -> Result<Rgba, String> {
    if (before.width, before.height) != (after.width, after.height) {
        let (b, a) = ((before.width, before.height), (after.width, after.height));
        return Err(format!("a wipe joins frames of one size, not {b:?} and {a:?}"));
    }
    let edge = (progress.clamp(0.0, 1.0) * before.width as f32).round() as usize * 4;
    let row = before.width as usize * 4;
    let mut pixels = before.pixels.clone();
    for (shown, from) in pixels.chunks_exact_mut(row).zip(after.pixels.chunks_exact(row)) {
        shown[..edge].copy_from_slice(&from[..edge]);
    }
    Ok(Rgba { width: before.width, height: before.height, pixels })
}

/// The wipe's frames over `seconds` at the film's rate, the edge eased: the first already moved from
/// the before held on the frame ahead of it, the last the after whole, where the film goes on.
pub fn frames(before: &Rgba, after: &Rgba, seconds: f32) -> Result<Vec<Rgba>, String> {
    let count = (seconds * FPS as f32).round().max(1.0) as u32;
    (1..=count).map(|i| wipe(before, after, ease(i as f32 / count as f32))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A frame where every pixel differs from its neighbours and from the other version's.
    fn frame(seed: u8) -> Rgba {
        let (width, height) = (240, 11);
        let pixels = (0..width * height * 4).map(|i| if i % 4 == 3 { 255 } else { (i as u8).wrapping_mul(7).wrapping_add(seed) }).collect();
        Rgba { width, height, pixels }
    }

    fn column(f: &Rgba, x: u32) -> Vec<u8> {
        (0..f.height).flat_map(|y| f.pixels[((y * f.width + x) * 4) as usize..][..4].to_vec()).collect()
    }

    #[test]
    fn the_wipe_runs_from_the_before_to_the_after_and_halfway_each_side_is_its_own_version() {
        let (before, after) = (frame(0), frame(101));
        assert!(wipe(&before, &after, 0.0).unwrap() == before, "progress 0 is the before");
        assert!(wipe(&before, &after, 1.0).unwrap() == after, "progress 1 is the after");
        let half = wipe(&before, &after, 0.5).unwrap();
        let edge = (0.5 * before.width as f32).round() as u32;
        for x in 0..before.width {
            let version = if x < edge { &after } else { &before };
            assert_eq!(column(&half, x), column(version, x), "column {x} is its version's to the bit");
        }
        // Twin: a wipe that kept either frame whole, or mixed them, would match one or neither side.
        assert!(half != before && half != after, "halfway differs from both");

        let shown = frames(&before, &after, 0.9).unwrap();
        assert_eq!(shown.len(), 18, "0.9 s at the film's rate");
        assert!(shown[0] != before && *shown.last().unwrap() == after, "it leaves the before and lands on the after");
        assert!(wipe(&before, &Rgba { width: 239, ..after.clone() }, 0.5).is_err(), "frames of two sizes are refused");
    }
}
