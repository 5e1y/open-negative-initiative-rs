//! Sharing: a scene's frame cut out of the window's own picture and written as a JPEG, 9 by 16 as a
//! phone's story takes it, at the pixels the screen drew.

use std::path::{Path, PathBuf};

/// High enough that flat inks and small type keep their edges.
pub const QUALITY: u8 = 95;

/// A rectangle of a picture, in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cut {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl Cut {
    /// A rectangle in points at `scale` pixels a point, its edges on whole pixels inside a picture of `within`.
    pub fn of(x: f32, y: f32, width: f32, height: f32, scale: f32, within: (usize, usize)) -> Option<Cut> {
        let (left, top) = ((x * scale).ceil().max(0.0) as usize, (y * scale).ceil().max(0.0) as usize);
        let right = (((x + width) * scale).floor().max(0.0) as usize).min(within.0);
        let bottom = (((y + height) * scale).floor().max(0.0) as usize).min(within.1);
        (right > left && bottom > top).then(|| Cut { x: left, y: top, width: right - left, height: bottom - top })
    }
}

/// The RGBA pixels of `cut` in a picture `width` wide; none when the picture does not hold it.
pub fn cut(rgba: &[u8], width: usize, cut: Cut) -> Option<Vec<u8>> {
    let height = rgba.len().checked_div(width * 4)?;
    if cut.x + cut.width > width || cut.y + cut.height > height {
        return None;
    }
    let mut out = Vec::with_capacity(cut.width * cut.height * 4);
    for row in cut.y..cut.y + cut.height {
        let start = (row * width + cut.x) * 4;
        out.extend_from_slice(&rgba[start..start + cut.width * 4]);
    }
    Some(out)
}

pub fn jpeg(rgba: &[u8], width: usize, height: usize) -> Option<Vec<u8>> {
    let (w, h) = (u16::try_from(width).ok()?, u16::try_from(height).ok()?);
    if width == 0 || height == 0 || rgba.len() != width * height * 4 {
        return None;
    }
    let mut out = Vec::new();
    jpeg_encoder::Encoder::new(&mut out, QUALITY).encode(rgba, w, h, jpeg_encoder::ColorType::Rgba).ok()?;
    Some(out)
}

/// A card's file name: the year and the scene's place in the story, from 1.
pub fn file_name(year: &str, scene: usize) -> String {
    format!("ONI Wrapped {year} - {:02}.jpg", scene + 1)
}

/// `name` in `folder`, or with the first number that makes it a file not there yet: a card saved
/// twice never replaces the first.
pub fn free(folder: &Path, name: &str) -> PathBuf {
    let (stem, extension) = name.rsplit_once('.').unwrap_or((name, ""));
    let mut path = folder.join(name);
    let mut turn = 2;
    while path.exists() {
        path = folder.join(format!("{stem} ({turn}).{extension}"));
        turn += 1;
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A picture whose every pixel holds its own column and row.
    fn picture(width: usize, height: usize) -> Vec<u8> {
        (0..height).flat_map(|y| (0..width).flat_map(move |x| [x as u8, y as u8, 0, 255])).collect()
    }

    #[test]
    fn a_frame_is_cut_on_whole_pixels_inside_the_window() {
        // 10.2 pt to 30.6 pt at 2 px a point: pixels 21 to 61, the half pixels at both edges left out.
        assert_eq!(Cut::of(10.2, 5.0, 20.4, 10.0, 2.0, (200, 100)), Some(Cut { x: 21, y: 10, width: 40, height: 20 }));
        assert_eq!(Cut::of(90.0, 0.0, 20.0, 60.0, 2.0, (200, 100)), Some(Cut { x: 180, y: 0, width: 20, height: 100 }), "held inside the picture");
        assert_eq!(Cut::of(120.0, 0.0, 20.0, 10.0, 2.0, (200, 100)), None, "wholly outside");
        let rgba = picture(8, 6);
        let piece = cut(&rgba, 8, Cut { x: 2, y: 1, width: 3, height: 2 }).unwrap();
        assert_eq!(piece, [2, 1, 0, 255, 3, 1, 0, 255, 4, 1, 0, 255, 2, 2, 0, 255, 3, 2, 0, 255, 4, 2, 0, 255]);
        // Twin: a cut past the picture's edge gives nothing rather than another row's pixels.
        assert_eq!(cut(&rgba, 8, Cut { x: 6, y: 1, width: 3, height: 2 }), None);
        assert_eq!(cut(&rgba, 8, Cut { x: 0, y: 5, width: 3, height: 2 }), None);
    }

    #[test]
    fn a_card_is_a_jpeg_of_its_own_size_and_never_replaces_another() {
        let rgba = picture(18, 32);
        let data = jpeg(&rgba, 18, 32).unwrap();
        let mut decoder = jpeg_decoder::Decoder::new(data.as_slice());
        decoder.decode().unwrap();
        let info = decoder.info().unwrap();
        assert_eq!((info.width, info.height), (18, 32));
        assert_eq!(jpeg(&rgba, 18, 31), None, "twin: a size its pixels do not make");
        assert_eq!(file_name("2026", 0), "ONI Wrapped 2026 - 01.jpg");
        let folder = std::env::temp_dir().join(format!("oni-wrapped-share-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let first = free(&folder, "card.jpg");
        assert_eq!(first, folder.join("card.jpg"));
        std::fs::write(&first, b"1").unwrap();
        let second = free(&folder, "card.jpg");
        assert_eq!(second, folder.join("card (2).jpg"));
        std::fs::write(&second, b"2").unwrap();
        assert_eq!(free(&folder, "card.jpg"), folder.join("card (3).jpg"));
        let _ = std::fs::remove_dir_all(&folder);
    }
}
