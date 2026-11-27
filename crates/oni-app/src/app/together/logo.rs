//! The signature's band across the sheet's bottom: #madewithoni a column wide at its left, at full scale.

use super::*;
use super::plate::{Face, Plate};
use swash::scale::ScaleContext;

/// The owner's own words, never translated, set in the brand's face.
const SIGNATURE: &str = "#madewithoni";

/// The band on a clear plate `width` wide and `BAND` tall: the signature a tile wide, no wider than
/// the plate, against its left edge and centred in its height, in `ink`.
pub(in crate::app) fn drawn(width: usize, ink: [f32; 3]) -> Plate {
    let mut plate = Plate::clear(width, BAND);
    let Some(face) = Face::display() else { return plate };
    let wide = face.advance(SIGNATURE, 1.0);
    let size = (TILE.min(width) as f32 / wide).min(BAND as f32);
    let baseline = face.centred(0.0, BAND as f32, size);
    plate.text_in(&face, (0.0, baseline), SIGNATURE, size, ink, &mut ScaleContext::new());
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
    fn the_signature_spans_the_first_column_centred_in_the_band_and_nothing_else_is_drawn() {
        let plate = drawn(5 * TILE + 4 * GAP, [1.0; 3]);
        let (columns, rows) = inked(&plate);
        let (left, right) = (columns[0], *columns.last().unwrap());
        assert!(left < 16 && (TILE - 40..=TILE).contains(&right), "the signature across the first column: {left}..{right}");
        let (top, bottom) = (rows[0], *rows.last().unwrap());
        assert!(bottom - top < BAND / 2 && (top as i64 - (BAND - 1 - bottom) as i64).abs() < 30, "a line centred in the band: rows {top}..{bottom}");
        let white = plate.pixels.iter().filter(|p| p[3] > 0.9 && p[0] > 0.9 && p[2] > 0.9).count();
        assert!(white > 20_000, "in its ink: {white} pixels");
        // Twin: in black it lights none; a sheet narrower than a tile sets it across it, smaller.
        assert!(!drawn(TILE, [0.0; 3]).pixels.iter().any(|p| p[0] > 0.0));
        let narrow = drawn(TILE / 2, [1.0; 3]);
        let (columns, rows) = inked(&narrow);
        assert!(*columns.last().unwrap() > TILE / 2 - 40 && rows.len() < (bottom - top) / 2 + 8, "{} wide, {} tall", columns.last().unwrap(), rows.len());
    }
}
