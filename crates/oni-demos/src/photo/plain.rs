//! A finished photograph as it is: a JPEG export read as the app reads a plain image, its code values
//! kept, never through the conversion; reduced by the app's own reduction and a window cut from it.

use std::path::Path;

use super::Rgba;

/// A window into a frame: the reduction's longest side, the window's centre across as a fraction of
/// the width, and its first row in the reduction.
#[derive(Clone, Copy, Debug)]
pub struct Cut {
    pub side: f64,
    pub centre: f64,
    pub top: usize,
}

/// `path` reduced to `cut.side` on its longest side by the app's own reduction, `width` × `height` cut
/// from it. An sRGB file's values are an sRGB screen's, so they are kept.
pub fn window(path: &Path, (width, height): (u32, u32), cut: Cut) -> Result<Rgba, String> {
    let read = oni_decode::plain::read(path, Some(cut.side)).ok_or_else(|| format!("{} does not decode", path.display()))?;
    // The profile's description names sRGB, in ASCII (ICC 2) or UTF-16 (ICC 4).
    let srgb = |icc: &Vec<u8>| icc.windows(4).any(|t| t == b"sRGB") || icc.windows(8).any(|t| t == b"\0s\0R\0G\0B");
    if read.icc.as_ref().is_some_and(|icc| !srgb(icc)) {
        return Err("the photograph's profile is not sRGB: its values are not the screen's".into());
    }
    let (bw, bh) = (width as usize, height as usize);
    if read.width < bw || read.height < cut.top + bh {
        return Err(format!("{} reduces to {} × {}, short of {width} × {height} from row {}", path.display(), read.width, read.height, cut.top));
    }
    let x0 = ((read.width as f64 * cut.centre).round() as usize).saturating_sub(bw / 2).min(read.width - bw);
    let mut pixels = Vec::with_capacity(bw * bh * 4);
    for y in cut.top..cut.top + bh {
        for p in &read.pixels[y * read.width + x0..][..bw] {
            pixels.extend(p[..3].iter().map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8));
            pixels.push(255);
        }
    }
    Ok(Rgba { width, height, pixels })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plain image of `w` × `h` whose left half is red and right half blue, naming no profile.
    fn halves(dir: &Path, w: u32, h: u32) -> std::path::PathBuf {
        let file = dir.join("halves.png");
        let pixels = (0..w * h).flat_map(|i| if i % w < w / 2 { [220, 30, 30, 255] } else { [30, 30, 220, 255] }).collect();
        Rgba { width: w, height: h, pixels }.png(&file).unwrap();
        file
    }

    #[test]
    fn an_export_is_shown_as_it_is_in_the_window_cut() {
        let dir = std::env::temp_dir().join(format!("oni-demos-plain-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = halves(&dir, 400, 200);
        let cut = |centre| Cut { side: 400.0, centre, top: 50 };
        let left = window(&file, (40, 100), cut(0.1)).unwrap();
        let right = window(&file, (40, 100), cut(0.9)).unwrap();
        assert_eq!((left.width, left.height, left.pixels.len()), (40, 100, 40 * 100 * 4));
        let at = |p: &Rgba, x: u32, y: u32| p.pixels[((y * p.width + x) * 4) as usize..][..4].to_vec();
        // The file's own code values come back: nothing but a reduction lies between file and screen.
        assert_eq!(at(&left, 20, 50), [220, 30, 30, 255]);
        assert_eq!(at(&right, 20, 50), [30, 30, 220, 255]);
        // Twin: the centre moves the cut, so the two boxes are not one picture.
        assert!(left != right);
        assert!(window(&file, (40, 100), Cut { top: 101, ..cut(0.1) }).is_err(), "a window past the foot is refused");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
