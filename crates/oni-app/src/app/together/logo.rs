//! The logo's band across the sheet's bottom: the logo a column wide at its left, at full scale.

use super::*;
use super::plate::Plate;
use resvg::{tiny_skia, usvg};

const LOGO_SVG: &[u8] = include_bytes!("../../../../oni-ui/ui/icons/oni-logo.svg");

/// The band on a clear plate `width` wide and `BAND` tall: the logo a tile wide, no wider than
/// the plate, against its left edge and centred in its height, in its own colours.
pub(in crate::app) fn drawn(width: usize) -> Plate {
    let mut plate = Plate::clear(width, BAND);
    let Ok(tree) = usvg::Tree::from_data(LOGO_SVG, &usvg::Options::default()) else { return plate };
    let size = tree.size();
    let scale = (TILE.min(width) as f32 / size.width()).min(BAND as f32 / size.height());
    let (w, h) = ((size.width() * scale).ceil() as u32, (size.height() * scale).ceil() as u32);
    let Some(mut pixmap) = tiny_skia::Pixmap::new(w, h) else { return plate };
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    let top = (BAND as i64 - i64::from(h)) / 2;
    for (i, p) in pixmap.pixels().iter().enumerate() {
        let c = p.demultiply();
        let ink = [c.red(), c.green(), c.blue()].map(|v| f32::from(v) / 255.0);
        plate.cover((i % w as usize) as i64, top + (i / w as usize) as i64, f32::from(c.alpha()) / 255.0, ink);
    }
    plate
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The columns and rows of `plate` holding any ink.
    fn inked(plate: &Plate) -> (Vec<usize>, Vec<usize>) {
        let on = |x: usize, y: usize| plate.pixels[y * plate.width + x][3] > 0.5;
        ((0..plate.width).filter(|&x| (0..plate.height).any(|y| on(x, y))).collect(), (0..plate.height).filter(|&y| (0..plate.width).any(|x| on(x, y))).collect())
    }

    #[test]
    fn the_logo_spans_the_first_column_centred_in_the_band_and_nothing_else_is_drawn() {
        let plate = drawn(5 * TILE + 4 * GAP);
        let (columns, rows) = inked(&plate);
        assert!(columns[0] < 4 && (TILE - 8..TILE).contains(columns.last().unwrap()), "the logo across the first column: {}..{}", columns[0], columns.last().unwrap());
        assert!(rows[0] < 12 && *rows.last().unwrap() > BAND - 12, "the logo the band's height: rows {}..{}", rows[0], rows.last().unwrap());
        let white = plate.pixels.iter().filter(|p| p[3] > 0.9 && p[0] > 0.9 && p[2] > 0.9).count();
        assert!(white > 20_000, "in its own white: {white} pixels");
        // Twin: a sheet narrower than a tile sets the logo across it, smaller.
        let narrow = drawn(TILE / 2);
        let (columns, rows) = inked(&narrow);
        assert!(*columns.last().unwrap() > TILE / 2 - 8 && rows.len() < BAND / 2 + 8, "{} wide, {} tall", columns.last().unwrap(), rows.len());
    }
}
