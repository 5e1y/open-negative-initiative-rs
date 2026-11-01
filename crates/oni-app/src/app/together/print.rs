//! Edit Together's sheet written as one file: each frame decoded at a tile's side and laid row by row
//! as the canvas lays it, on the sheet's black, each number set under its picture as `EdgePrint` sets it.

use super::*;
use super::numbers::under;
use crate::export::Target;
use crate::render::Laid;
use oni_export::{Failure, Image as Picture};
use std::sync::atomic::AtomicBool;
use swash::scale::{Render, ScaleContext, Source as Outline};
use swash::zeno::Format as Mask;

/// The interface's face, which the numbers are set in on screen.
const FACE: &[u8] = include_bytes!("../../../../oni-ui/fonts/Inter.ttf");

/// The numbers' ink as the design system states it, in sRGB, and their weight.
#[derive(Clone, Copy, Debug)]
pub(in crate::app) struct Lettering {
    pub ink: [f32; 3],
    pub weight: f32,
}

/// The sheet of `targets` at full scale in the working space, numbered by `numbers`; `None` once
/// `stop` is raised. Rows are drawn one at a time, so no texture outgrows the GPU's side.
pub(in crate::app) fn printed(engine: &Shared, targets: &[Target], numbers: &[Option<usize>], lettering: Lettering, stop: &AtomicBool) -> Result<Option<Picture>, Failure> {
    let stopped = || stop.load(Ordering::Relaxed);
    let Some(negatives) = decoded(engine, targets, &stopped)? else { return Ok(None) };
    let proportions: Vec<(f64, f64)> =
        negatives.iter().zip(targets).map(|(n, t)| crate::export::cropped_size((f64::from(n.size().0), f64::from(n.size().1)), &t.settings)).collect();
    let sheet = laid(&proportions);
    let (width, height) = sheet.drawn(1.0);
    let mut picture = black(width as usize, height as usize);
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
        let drawn = e.render_laid(&present, TILE, &at, (ww, wh)).map_err(|_| Failure::Render)?;
        let band = crate::export::read_laid(&mut e, drawn)?;
        drop(e);
        laid_on(&mut picture, &band, (wx as usize, wy as usize));
    }
    let mut context = ScaleContext::new();
    for (x, y, n) in under(numbers, &sheet.pictures) {
        letter(&mut picture, (x, y), &n.to_string(), lettering, &mut context);
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

/// An opaque black sheet.
fn black(width: usize, height: usize) -> Picture {
    let pixels = std::iter::repeat_n([0.0, 0.0, 0.0, 1.0], width * height).flatten().collect();
    Picture { width, height, pixels }
}

/// `band` laid on `sheet` at `at` over its black: a clear pixel, a gap between tiles, stays black.
fn laid_on(sheet: &mut Picture, band: &Picture, at: (usize, usize)) {
    for y in 0..band.height.min(sheet.height.saturating_sub(at.1)) {
        for x in 0..band.width.min(sheet.width.saturating_sub(at.0)) {
            let s = &band.pixels[(y * band.width + x) * 4..][..4];
            let a = s[3].clamp(0.0, 1.0);
            let d = &mut sheet.pixels[((at.1 + y) * sheet.width + at.0 + x) * 4..][..4];
            d.copy_from_slice(&[s[0] * a, s[1] * a, s[2] * a, 1.0]);
        }
    }
}

/// `text` set in the interface's face at `NUMBER` pixels from the left of `at`, centred in the `MARGIN`
/// band under it, blended over the black in sRGB as the screen blends it, then linearised.
fn letter(sheet: &mut Picture, at: (f64, f64), text: &str, lettering: Lettering, context: &mut ScaleContext) {
    let Some(font) = swash::FontRef::from_index(FACE, 0) else { return };
    let axes = [(b"wght", lettering.weight)];
    let coords: Vec<_> = font.variations().normalized_coords(&axes).collect();
    let size = NUMBER as f32;
    let metrics = font.metrics(&coords).scale(size);
    let advances = font.glyph_metrics(&coords).scale(size);
    let baseline = at.1 as f32 + (MARGIN as f32 - (metrics.ascent + metrics.descent)) / 2.0 + metrics.ascent;
    let mut scaler = context.builder(font).size(size).normalized_coords(&coords).build();
    let mut pen = at.0 as f32;
    let srgb = oni_colour::source::Source::of(None, None, false);
    for c in text.chars() {
        let glyph = font.charmap().map(c);
        let Some(mask) = Render::new(&[Outline::Outline]).format(Mask::Alpha).render(&mut scaler, glyph) else { continue };
        let p = mask.placement;
        let (left, top) = ((pen.round() as i32 + p.left) as i64, (baseline.round() as i32 - p.top) as i64);
        let mut covered: Vec<[f32; 4]> = mask.data.iter().map(|&a| lettering.ink.map(|v| v * f32::from(a) / 255.0)).map(|[r, g, b]| [r, g, b, 1.0]).collect();
        srgb.linearise(&mut covered, false);
        for (i, (&a, ink)) in mask.data.iter().zip(&covered).enumerate() {
            let (x, y) = (left + (i % p.width as usize) as i64, top + (i / p.width as usize) as i64);
            if a == 0 || x < 0 || y < 0 || x >= sheet.width as i64 || y >= sheet.height as i64 {
                continue;
            }
            let d = &mut sheet.pixels[(y as usize * sheet.width + x as usize) * 4..][..3];
            // Over black, a glyph's edge is its ink at its coverage; two glyphs touching keep the brighter.
            for (d, v) in d.iter_mut().zip(ink) {
                *d = d.max(*v);
            }
        }
        pen += advances.advance_width(glyph);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORANGE: Lettering = Lettering { ink: [1.0, 159.0 / 255.0, 10.0 / 255.0], weight: 700.0 };

    /// The rows of `p` holding any lit pixel, and its lit pixels' count.
    fn lit(p: &Picture) -> (Vec<usize>, usize) {
        let on = |i: usize| p.pixels[i * 4..i * 4 + 3].iter().any(|v| *v > 1e-4);
        let rows = (0..p.height).filter(|y| (0..p.width).any(|x| on(y * p.width + x))).collect();
        (rows, (0..p.width * p.height).filter(|i| on(*i)).count())
    }

    #[test]
    fn a_number_is_set_in_the_band_under_its_picture_orange_on_black() {
        let mut sheet = black(400, 200);
        letter(&mut sheet, (20.0, 40.0), "12", ORANGE, &mut ScaleContext::new());
        let (rows, count) = lit(&sheet);
        assert!(count > 200, "the digits are drawn: {count} pixels");
        assert!(rows[0] >= 40 && *rows.last().unwrap() < 40 + MARGIN, "within the band: rows {}..{}", rows[0], rows.last().unwrap());
        // Its fullest pixel is the ink linearised: red above green above blue, opaque.
        let brightest = sheet.pixels.chunks_exact(4).max_by(|a, b| a[0].total_cmp(&b[0])).unwrap();
        assert!(brightest[0] > brightest[1] && brightest[1] > brightest[2] && brightest[3] == 1.0, "{brightest:?}");
        // Twin: the regular weight lights fewer pixels than the bold one.
        let mut regular = black(400, 200);
        letter(&mut regular, (20.0, 40.0), "12", Lettering { weight: 400.0, ..ORANGE }, &mut ScaleContext::new());
        assert!(lit(&regular).1 < count, "a regular number as heavy as a bold one");
    }

    #[test]
    fn a_band_lies_on_the_black_its_gaps_left_black() {
        let mut sheet = black(4, 2);
        let band = Picture { width: 2, height: 1, pixels: vec![0.5, 0.4, 0.3, 1.0, 0.9, 0.9, 0.9, 0.0] };
        laid_on(&mut sheet, &band, (1, 1));
        assert_eq!(&sheet.pixels[(4 + 1) * 4..(4 + 2) * 4], &[0.5, 0.4, 0.3, 1.0], "a picture's pixel lands at its place");
        assert_eq!(&sheet.pixels[(4 + 2) * 4..(4 + 3) * 4], &[0.0, 0.0, 0.0, 1.0], "a clear one stays black, opaque");
        // Twin: nothing lands outside the band's place.
        assert!(sheet.pixels[..4 * 4].chunks_exact(4).all(|p| p == [0.0, 0.0, 0.0, 1.0]));
    }
}
