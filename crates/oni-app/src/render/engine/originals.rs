//! The dust each correction covers, as the frame shows it without any patch: the De-dust tab's reminders.

use super::*;

/// A reminder's margin about its stroke, as a share of the stroke's own side.
const MARGIN: f64 = 0.6;
/// The smallest side a reminder covers, in the preview's pixels, so a speck still shows its surround.
const LEAST: f64 = 24.0;
/// A reminder's side once drawn: the tab's 56 points on a screen of two pixels a point.
const SHOWN: f64 = 112.0;

impl Engine {
    /// For each stroke (points in fractions of the rendered frame, y down, and a radius in full-resolution
    /// pixels), the square of the frame about it rendered with no correction at all.
    pub fn originals(&mut self, negative: &Negative, settings: &PipelineSettings, strokes: &[(Vec<(f64, f64)>, f64)]) -> Result<Vec<Linear>, String> {
        let mut bare = settings.clone();
        bare.corrections.clear();
        self.read_custom(&bare);
        let request = Request { scale: 1.0, full_width: Some(negative.full_width), ..Request::default() };
        let inputs = Inputs { patches: patches_for(&self.patches, &bare), custom_flat_field: custom_for(&self.custom, &bare) };
        let image = self.pipeline.image(&mut self.graph, &negative.preview, &bare, &request, &inputs).map_err(|e| e.to_string())?;
        let image = self.proof.laid(&mut self.graph, image, &bare)?;
        let e = image.extent();
        let per_full = f64::from(negative.size().0) / negative.full_width;
        let mut out = Vec::with_capacity(strokes.len());
        for (points, radius) in strokes {
            let r = reminder(points, radius * per_full, e);
            let mut cut = image.clone().then(Crop(r));
            if f64::from(r.w) > SHOWN {
                cut = cut.lanczos(SHOWN / f64::from(r.w));
            }
            let shown = cut.extent();
            let pixels = self.graph.render(&cut, shown, None).map_err(|e| e.to_string())?;
            out.push(Linear { width: shown.w as usize, height: shown.h as usize, pixels });
        }
        Ok(out)
    }
}

/// The square about a stroke's box and its brush, with a margin, kept inside the extent `e`.
pub(super) fn reminder(points: &[(f64, f64)], radius: f64, e: Rect) -> Rect {
    let (w, h) = (f64::from(e.w), f64::from(e.h));
    let xs = points.iter().map(|p| p.0 * w);
    let ys = points.iter().map(|p| p.1 * h);
    let (x0, x1) = xs.fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(v), b.max(v)));
    let (y0, y1) = ys.fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(v), b.max(v)));
    let (x0, x1, y0, y1) = if x0 <= x1 { (x0, x1, y0, y1) } else { (w / 2.0, w / 2.0, h / 2.0, h / 2.0) };
    let side = ((x1 - x0).max(y1 - y0) + 2.0 * radius) * (1.0 + MARGIN);
    let side = side.max(LEAST).min(w.min(h)).round().max(1.0);
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let left = (cx - side / 2.0).clamp(0.0, w - side).round() as i32;
    let top = (cy - side / 2.0).clamp(0.0, h - side).round() as i32;
    Rect::new(e.x + left, e.y + top, side as u32, side as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reminder_is_a_square_about_its_stroke_inside_the_frame() {
        let e = Rect::new(0, 0, 2000, 1333);
        let r = reminder(&[(0.5, 0.5), (0.52, 0.5)], 10.0, e);
        assert_eq!(r.w, r.h);
        assert_eq!(r.w, ((40.0 + 20.0) * 1.6f64).round() as u32, "the stroke's 40 px, the brush on each side, the margin");
        assert!((f64::from(r.x) + f64::from(r.w) / 2.0 - 1020.0).abs() <= 1.0, "centred on the stroke's box: {r:?}");
        // Twin: a stroke on the corner is pushed inside rather than cut.
        let corner = reminder(&[(0.0, 0.0), (0.01, 0.0)], 10.0, e);
        assert_eq!((corner.x, corner.y), (0, 0));
        assert!(e.contains(&reminder(&[(1.0, 1.0)], 300.0, e)));
        // A speck still shows its surround.
        assert_eq!(reminder(&[(0.5, 0.5)], 1.0, e).w, LEAST as u32);
    }
}
