//! Sharing: a scene's card taken off the window's own picture, 1080 by 1920 as a phone's story takes it,
//! a window's worth at a time; written as a JPEG into the year's folder, or handed to the pasteboard.

use std::path::{Path, PathBuf};

/// A story's picture, in pixels.
pub const CARD: (usize, usize) = (1080, 1920);
/// High enough that flat inks and small type keep their edges.
pub const QUALITY: u8 = 95;

/// The corners of the windows' worths a picture `card` large is taken in, a window being `window` large,
/// row by row.
pub fn tiles(card: (usize, usize), window: (usize, usize)) -> Vec<(usize, usize)> {
    if window.0 == 0 || window.1 == 0 {
        return Vec::new();
    }
    (0..card.1).step_by(window.1).flat_map(|y| (0..card.0).step_by(window.0).map(move |x| (x, y))).collect()
}

/// The view's part of a window's picture `shot`, `shot_width` wide, laid into the card `size` large with
/// its corner at `origin`: what of it falls inside the card and the picture, the rest left out. `view`
/// is the view's corner in the picture, then its size.
pub fn lay(card: &mut [u8], size: (usize, usize), origin: (usize, usize), shot: &[u8], shot_width: usize, view: ((usize, usize), (usize, usize))) {
    let shot_height = shot.len().checked_div(shot_width * 4).unwrap_or(0);
    let ((left, top), (across, down)) = view;
    let width = across.min(shot_width.saturating_sub(left)).min(size.0.saturating_sub(origin.0));
    let height = down.min(shot_height.saturating_sub(top)).min(size.1.saturating_sub(origin.1));
    for row in 0..height {
        let from = ((top + row) * shot_width + left) * 4;
        let to = ((origin.1 + row) * size.0 + origin.0) * 4;
        card[to..to + width * 4].copy_from_slice(&shot[from..from + width * 4]);
    }
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

/// The picture as a TIFF, the form the Mac's pasteboard hands every app that pastes an image.
pub fn tiff(rgba: &[u8], width: usize, height: usize) -> Option<Vec<u8>> {
    if width == 0 || height == 0 || rgba.len() != width * height * 4 {
        return None;
    }
    let rgb: Vec<u8> = rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
    let mut out = std::io::Cursor::new(Vec::new());
    tiff::encoder::TiffEncoder::new(&mut out).and_then(|mut e| e.write_image::<tiff::encoder::colortype::RGB8>(width as u32, height as u32, &rgb)).ok()?;
    Some(out.into_inner())
}

/// The picture as Windows' device-independent bitmap, the clipboard's own form: a 40-byte header, then
/// the rows from the last to the first, each pixel blue, green, red and a byte left at zero.
pub fn dib(rgba: &[u8], width: usize, height: usize) -> Option<Vec<u8>> {
    let (w, h) = (i32::try_from(width).ok()?, i32::try_from(height).ok()?);
    if width == 0 || height == 0 || rgba.len() != width * height * 4 {
        return None;
    }
    let mut out = Vec::with_capacity(40 + rgba.len());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&w.to_le_bytes());
    out.extend_from_slice(&h.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(rgba.len() as u32).to_le_bytes());
    out.extend_from_slice(&[0; 16]);
    for row in rgba.chunks_exact(width * 4).rev() {
        out.extend(row.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], 0]));
    }
    Some(out)
}

/// The folder a year's cards are saved in, inside the person's Downloads.
pub fn folder(downloads: &Path, year: i32) -> PathBuf {
    downloads.join(folder_name(year))
}

pub fn folder_name(year: i32) -> String {
    format!("ONI Wrapped {year}")
}

/// A card's file name: the year and the scene's place in the story, from 1. A card saved again takes
/// the place of the one saved before, in the folder that is the cards' own.
pub fn file_name(year: i32, scene: usize) -> String {
    format!("ONI Wrapped {year} - {:02}.jpg", scene + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A picture whose every pixel holds its own column and row.
    fn picture(width: usize, height: usize) -> Vec<u8> {
        (0..height).flat_map(|y| (0..width).flat_map(move |x| [x as u8, y as u8, 7, 255])).collect()
    }

    #[test]
    fn a_card_larger_than_the_window_is_taken_a_windows_worth_at_a_time() {
        // A Retina window of 2200 by 1360 pixels holds a card's width at once and its height in two.
        assert_eq!(tiles(CARD, (2200, 1360)), [(0, 0), (0, 1360)]);
        assert_eq!(tiles(CARD, (1280, 800)), [(0, 0), (0, 800), (0, 1600)]);
        assert_eq!(tiles((10, 10), (4, 6)), [(0, 0), (4, 0), (8, 0), (0, 6), (4, 6), (8, 6)]);
        assert_eq!(tiles(CARD, (4000, 4000)), [(0, 0)]);
        assert!(tiles(CARD, (0, 800)).is_empty());
        // Each picture's view is laid where it was taken: together they are the card, to the pixel. The
        // window holds a menu bar two rows deep and a margin a column wide, which no card shows.
        let (card, view, bar) = ((10, 7), (4, 3), (1, 2));
        let window = (view.0 + bar.0, view.1 + bar.1);
        let whole = picture(card.0, card.1);
        let mut laid = vec![0u8; card.0 * card.1 * 4];
        // What the window shows with the card's corner drawn at the view's, less (x, y): the card from
        // (x, y) on inside the view, 99 around it.
        let shown = |x: usize, y: usize| -> Vec<u8> {
            (0..window.1).flat_map(|r| (0..window.0).flat_map(move |c| if c < bar.0 || r < bar.1 { [99, 99, 99, 255] } else { [(x + c - bar.0) as u8, (y + r - bar.1) as u8, 7, 255] })).collect()
        };
        for (x, y) in tiles(card, view) {
            lay(&mut laid, card, (x, y), &shown(x, y), window.0, (bar, view));
        }
        assert_eq!(laid, whole);
        // Twin: taken from the window's own corner, the bar is in the card.
        let mut wrong = vec![0u8; card.0 * card.1 * 4];
        lay(&mut wrong, card, (0, 0), &shown(0, 0), window.0, ((0, 0), view));
        assert_eq!(&wrong[..4], &[99, 99, 99, 255]);
        assert_ne!(wrong[..view.0 * 4], whole[..view.0 * 4]);
    }

    #[test]
    fn a_card_is_a_jpeg_of_its_own_size_in_the_years_folder() {
        let rgba = picture(18, 32);
        let data = jpeg(&rgba, 18, 32).unwrap();
        let mut decoder = jpeg_decoder::Decoder::new(data.as_slice());
        decoder.decode().unwrap();
        let info = decoder.info().unwrap();
        assert_eq!((info.width, info.height), (18, 32));
        assert_eq!(jpeg(&rgba, 18, 31), None, "twin: a size its pixels do not make");
        assert_eq!(CARD.0 * 16, CARD.1 * 9, "a story's 9 by 16");
        assert_eq!(file_name(2026, 0), "ONI Wrapped 2026 - 01.jpg");
        assert_eq!(folder(Path::new("/home/Downloads"), 2026), Path::new("/home/Downloads/ONI Wrapped 2026"));
    }

    #[test]
    fn a_card_is_handed_to_the_pasteboard_as_a_tiff_and_as_a_bitmap() {
        let rgba = picture(3, 2);
        let data = tiff(&rgba, 3, 2).unwrap();
        let mut decoder = tiff::decoder::Decoder::new(std::io::Cursor::new(&data)).unwrap();
        assert_eq!(decoder.dimensions().unwrap(), (3, 2));
        let tiff::decoder::DecodingResult::U8(pixels) = decoder.read_image().unwrap() else { panic!("eight bits a channel") };
        assert_eq!(pixels, [0, 0, 7, 1, 0, 7, 2, 0, 7, 0, 1, 7, 1, 1, 7, 2, 1, 7]);
        let bitmap = dib(&rgba, 3, 2).unwrap();
        assert_eq!(bitmap.len(), 40 + 24);
        assert_eq!((&bitmap[..4], &bitmap[4..8], &bitmap[8..12], &bitmap[14..16]), (&[40, 0, 0, 0][..], &[3, 0, 0, 0][..], &[2, 0, 0, 0][..], &[32, 0][..]));
        // The last row first, blue before red: the picture's lower left pixel is (0, 1, 7).
        assert_eq!(&bitmap[40..44], &[7, 1, 0, 0]);
        assert_ne!(&bitmap[40..44], &[0, 0, 7, 0], "twin: the first row's pixel, red first, is not what a bitmap starts with");
        assert_eq!((tiff(&rgba, 3, 3), dib(&rgba, 0, 2)), (None, None));
    }
}
