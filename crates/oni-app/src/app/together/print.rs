//! Edit Together's sheet written as one file: each frame decoded at a tile's side and laid row by row
//! as the canvas lays it, on the sheet's ground above its logo, each number set as `EdgePrint` sets it.

use super::*;
use super::numbers::under;
use crate::export::Target;
use crate::render::Laid;
use oni_export::{Failure, Image as Picture};
use std::sync::atomic::AtomicBool;
use super::plate::{Face, Inks, Lettering, Plate, linear};
use swash::scale::ScaleContext;

/// What a sheet shows beside its frames: each one's number, and the sheet's inks.
pub(in crate::app) struct Captions {
    pub numbers: Vec<Option<usize>>,
    pub inks: Inks,
}

/// The sheet of `targets` at full scale in the working space, laid on `sizes` (each frame's cropped
/// size, else its tile's) with its `captions`; `None` once `stop` is raised.
pub(in crate::app) fn printed(engine: &Shared, targets: &[Target], sizes: Option<Vec<(f64, f64)>>, captions: &Captions, stop: &AtomicBool) -> Result<Option<Picture>, Failure> {
    let stopped = || stop.load(Ordering::Relaxed);
    let Some(negatives) = decoded(engine, targets, &stopped)? else { return Ok(None) };
    let tiles = || negatives.iter().zip(targets).map(|(n, t)| crate::export::cropped_size((f64::from(n.size().0), f64::from(n.size().1)), &t.settings)).collect();
    let sheet = laid(&sizes.filter(|s| s.len() == targets.len()).unwrap_or_else(tiles));
    // Rows are drawn one at a time, so no texture outgrows the GPU's side.
    let (width, height) = sheet.drawn(1.0);
    let ground = captions.inks.ground;
    let mut picture = filled(width as usize, height as usize, linear(ground));
    for row in (0..targets.len()).collect::<Vec<_>>().chunks(COLUMNS) {
        if stopped() {
            return Ok(None);
        }
        let top = row.iter().map(|&i| sheet.pictures[i].1).fold(f64::MAX, f64::min);
        let bottom = row.iter().map(|&i| sheet.pictures[i].1 + sheet.pictures[i].3).fold(0.0, f64::max);
        let (_, (wx, wy, ww, wh)) = sheet.seen(1.0, (0.0, top, sheet.size.0, bottom - top));
        let present: Vec<Laid> = row.iter().map(|&i| Laid::Frame(&negatives[i], &targets[i].settings)).collect();
        let at = |k: usize, tile| {
            let (x, y) = sheet.place(row[k], tile, 1.0);
            (x - wx as i32, y - wy as i32)
        };
        let mut e = engine.lock().map_err(|_| Failure::Render)?;
        let drawn = e.render_laid(&present, TILE, &at, (ww, wh)).map_err(|error| {
            crate::timing::line(&format!("contact sheet: row of {:?} at {:?} not drawn: {error}", row, (wx, wy, ww, wh)));
            Failure::Render
        })?;
        let band = crate::export::read_laid(&mut e, drawn)?;
        drop(e);
        laid_on(&mut picture, &band, (wx as usize, wy as usize));
    }
    let mut context = ScaleContext::new();
    for (x, y, n) in under(&captions.numbers, &sheet.pictures) {
        number(&mut picture, (x, y), n, captions.inks.numbers, ground, &mut context);
    }
    if let Some((bx, by, bw, _)) = sheet.band {
        super::logo::drawn(bw as usize, captions.inks.logo).lay_on(&mut picture, (bx as i64, by as i64), ground);
    }
    Ok(Some(picture))
}

/// Each frame at a tile's side, as many decoding at once as the decode gate admits; `None` once stopped.
fn decoded(engine: &Shared, targets: &[Target], stopped: &(dyn Fn() -> bool + Sync)) -> Result<Option<Vec<Negative>>, Failure> {
    let next = AtomicUsize::new(0);
    let slots: Vec<std::sync::Mutex<Option<Result<Negative, String>>>> = targets.iter().map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|scope| {
        for _ in 0..crate::decode_gate::DECODES.min(targets.len()) {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(t) = targets.get(i).filter(|_| !stopped()) else { return };
                    let path = oni_library::paths::normal(&t.path.to_string_lossy()).into_owned();
                    let made = drawing::tile_decoded(engine, &path, &t.settings, || !stopped());
                    if let Ok(mut slot) = slots[i].lock() {
                        *slot = Some(made);
                    }
                }
            });
        }
    });
    if stopped() {
        return Ok(None);
    }
    slots.into_iter().map(|s| s.into_inner().ok().flatten().and_then(Result::ok).ok_or(Failure::Decode)).collect::<Result<Vec<_>, _>>().map(Some)
}

/// An opaque sheet of `ground`, in working light.
fn filled(width: usize, height: usize, ground: [f32; 3]) -> Picture {
    let pixels = std::iter::repeat_n([ground[0], ground[1], ground[2], 1.0], width * height).flatten().collect();
    Picture { width, height, pixels }
}

/// `band` laid on `sheet` at `at` over its ground: a clear pixel, a gap between tiles, keeps it.
fn laid_on(sheet: &mut Picture, band: &Picture, at: (usize, usize)) {
    for y in 0..band.height.min(sheet.height.saturating_sub(at.1)) {
        for x in 0..band.width.min(sheet.width.saturating_sub(at.0)) {
            let s = &band.pixels[(y * band.width + x) * 4..][..4];
            let a = s[3].clamp(0.0, 1.0);
            let d = &mut sheet.pixels[((at.1 + y) * sheet.width + at.0 + x) * 4..][..4];
            let over = [0, 1, 2].map(|c| s[c] * a + d[c] * (1.0 - a));
            d.copy_from_slice(&[over[0], over[1], over[2], 1.0]);
        }
    }
}

/// Frame `n`'s number at `NUMBER` pixels from the left of `at`, centred in the `MARGIN` band under it.
fn number(sheet: &mut Picture, at: (f64, f64), n: usize, lettering: Lettering, ground: [f32; 3], context: &mut ScaleContext) {
    let Some(face) = Face::of(lettering.weight) else { return };
    let mut plate = Plate::clear(6 * NUMBER, MARGIN);
    plate.text((0.0, face.centred(0.0, MARGIN as f32, NUMBER as f32)), &n.to_string(), NUMBER as f32, lettering, context);
    plate.lay_on(sheet, (at.0.round() as i64, at.1.round() as i64), ground);
}

#[cfg(test)]
mod tests {
    use super::super::plate::ORANGE;
    use super::*;

    /// The rows of `p` holding any lit pixel, and its lit pixels' count.
    fn lit(p: &Picture) -> (Vec<usize>, usize) {
        let on = |i: usize| p.pixels[i * 4..i * 4 + 3].iter().any(|v| *v > 1e-4);
        let rows = (0..p.height).filter(|y| (0..p.width).any(|x| on(y * p.width + x))).collect();
        (rows, (0..p.width * p.height).filter(|i| on(*i)).count())
    }

    #[test]
    fn a_number_is_set_in_the_band_under_its_picture_orange_on_black() {
        let mut sheet = filled(400, 200, [0.0; 3]);
        number(&mut sheet, (20.0, 40.0), 12, ORANGE, [0.0; 3], &mut ScaleContext::new());
        let (rows, count) = lit(&sheet);
        assert!(count > 200, "the digits are drawn: {count} pixels");
        assert!(rows[0] >= 40 && *rows.last().unwrap() < 40 + MARGIN, "within the band: rows {}..{}", rows[0], rows.last().unwrap());
        // Its fullest pixel is the ink linearised: red above green above blue, opaque.
        let brightest = sheet.pixels.chunks_exact(4).max_by(|a, b| a[0].total_cmp(&b[0])).unwrap();
        assert!(brightest[0] > brightest[1] && brightest[1] > brightest[2] && brightest[3] == 1.0, "{brightest:?}");
        // Twin: the regular weight lights fewer pixels than the bold one.
        let mut regular = filled(400, 200, [0.0; 3]);
        number(&mut regular, (20.0, 40.0), 12, Lettering { weight: 400.0, ..ORANGE }, [0.0; 3], &mut ScaleContext::new());
        assert!(lit(&regular).1 < count, "a regular number as heavy as a bold one");
    }

    #[test]
    fn a_band_lies_on_the_ground_its_gaps_keeping_it() {
        let mut sheet = filled(4, 2, [0.0; 3]);
        let band = Picture { width: 2, height: 1, pixels: vec![0.5, 0.4, 0.3, 1.0, 0.9, 0.9, 0.9, 0.0] };
        laid_on(&mut sheet, &band, (1, 1));
        assert_eq!(&sheet.pixels[(4 + 1) * 4..(4 + 2) * 4], &[0.5, 0.4, 0.3, 1.0], "a picture's pixel lands at its place");
        assert_eq!(&sheet.pixels[(4 + 2) * 4..(4 + 3) * 4], &[0.0, 0.0, 0.0, 1.0], "a clear one stays black, opaque");
        // On white, a clear pixel stays white.
        let mut white = filled(4, 2, [1.0; 3]);
        laid_on(&mut white, &band, (1, 1));
        assert_eq!(&white.pixels[(4 + 2) * 4..(4 + 3) * 4], &[1.0; 4]);
        // Twin: nothing lands outside the band's place.
        assert!(sheet.pixels[..4 * 4].chunks_exact(4).all(|p| p == [0.0, 0.0, 0.0, 1.0]));
    }
}
