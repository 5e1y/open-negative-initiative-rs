//! A finished photograph as it is: a JPEG export read as the app reads a plain image, its code values
//! kept, never through the conversion; reduced by the app's own reduction and cut to cover a box.

use std::path::Path;

use super::Rgba;

/// `path` covering `width` × `height` pixels, reduced until its short side meets the box, cut about
/// `centre`, a fraction of its width. An sRGB file's values are an sRGB screen's, so they are kept.
pub fn cover(path: &Path, (width, height): (u32, u32), centre: f64) -> Result<Rgba, String> {
    let (w, h) = oni_decode::plain::size(path).ok_or_else(|| format!("{} reads as no image", path.display()))?;
    let scale = (f64::from(width) / w as f64).max(f64::from(height) / h as f64);
    let read = oni_decode::plain::read(path, Some(w.max(h) as f64 * scale)).ok_or_else(|| format!("{} does not decode", path.display()))?;
    // The profile's description names sRGB, in ASCII (ICC 2) or UTF-16 (ICC 4).
    let srgb = |icc: &Vec<u8>| icc.windows(4).any(|t| t == b"sRGB") || icc.windows(8).any(|t| t == b"\0s\0R\0G\0B");
    if read.icc.as_ref().is_some_and(|icc| !srgb(icc)) {
        return Err("the photograph's profile is not sRGB: its values are not the screen's".into());
    }
    let (bw, bh) = (width as usize, height as usize);
    if read.width < bw || read.height < bh {
        return Err(format!("{} reduces to {} × {}, short of {width} × {height}", path.display(), read.width, read.height));
    }
    let x0 = ((read.width as f64 * centre).round() as usize).saturating_sub(bw / 2).min(read.width - bw);
    let y0 = (read.height - bh) / 2;
    let mut pixels = Vec::with_capacity(bw * bh * 4);
    for y in y0..y0 + bh {
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
    fn an_export_is_shown_as_it_is_cut_about_its_centre() {
        let dir = std::env::temp_dir().join(format!("oni-demos-plain-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = halves(&dir, 400, 200);
        let left = cover(&file, (40, 100), 0.1).unwrap();
        let right = cover(&file, (40, 100), 0.9).unwrap();
        assert_eq!((left.width, left.height, left.pixels.len()), (40, 100, 40 * 100 * 4));
        let at = |p: &Rgba, x: u32, y: u32| p.pixels[((y * p.width + x) * 4) as usize..][..4].to_vec();
        // The file's own code values come back: nothing but a reduction lies between file and screen.
        assert_eq!(at(&left, 20, 50), [220, 30, 30, 255]);
        assert_eq!(at(&right, 20, 50), [30, 30, 220, 255]);
        // Twin: the centre moves the cut, so the two boxes are not one picture.
        assert!(left != right);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
