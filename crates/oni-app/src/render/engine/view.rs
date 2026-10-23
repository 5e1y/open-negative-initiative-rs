//! The canvas's render: the region the view shows, at the scale it shows it, kept as one texture.

use super::*;
use oni_i18n::tr;

impl Engine {
    /// `Pipeline.apply` left on the GPU in one texture for the display kernel to read. With a view,
    /// only its visible region, from the full frame once built, else the preview, each reduced once to
    /// the scale shown, so a notch computes no hidden pixel.
    pub fn resident(&mut self, negative: &Negative, settings: &PipelineSettings, view: Option<&View>) -> Result<(Source, Option<Placed>), String> {
        let request = Request { full_width: Some(negative.full_width), ..Request::default() };
        self.read_custom(settings);
        let Some(view) = view else {
            let inputs = Inputs { patches: patches_for(&self.patches, settings), custom_flat_field: custom_for(&self.custom, settings) };
            let image = self.pipeline.image(&mut self.graph, &negative.preview, settings, &request, &inputs).map_err(|e| e.to_string())?;
            let image = self.proof.laid(&mut self.graph, image, settings)?;
            return Ok((self.one_texture(&image, None)?, None));
        };
        let ratio = negative.size().0 as f64 / negative.full_width;
        let shown = {
            let inputs = Inputs { patches: patches_for(&self.patches, settings), custom_flat_field: custom_for(&self.custom, settings) };
            self.pipeline.image(&mut self.graph, &negative.preview, settings, &request, &inputs).map_err(|e| e.to_string())?.extent()
        };
        let full = (shown.w as f64 / ratio, shown.h as f64 / ratio);
        let vp = view.viewport;
        let scale = vp.effective_scale(full, view.size);
        let placement = vp.placement(full, view.size);
        let (base, base_ratio) = match &view.full {
            Some(f) => (f.clone(), 1.0),
            None => (negative.preview.clone(), ratio),
        };
        let e = base.extent();
        let long = e.w.max(e.h) as usize;
        // While a hand moves the view the region is rendered at most GESTURE_PIXELS and shown stretched,
        // so a 5K canvas costs a gesture what a small window does; the sharp render follows the stop.
        let margins = (1.0 + 2.0 * PAN_MARGIN) * (1.0 + 2.0 * PAN_MARGIN);
        let lighter = if view.coarse { (GESTURE_PIXELS / (view.size.0 * view.size.1 * margins)).sqrt().min(1.0) } else { 1.0 };
        let side = ((long as f64 * (scale * lighter / base_ratio).min(1.0)).round() as usize).max(1);
        // Reduced on the way in, so the chain pulls only the region's share of the level it reads; a
        // reduction the canvas nearly holds whole is kept, so a notch at fit reads it ready.
        let (source, reduce) = match side < long {
            true => {
                let (level, reduce) = self.level(&base, side, long)?;
                let e = level.extent();
                let whole = f64::from(e.w) * f64::from(e.h) * reduce * reduce;
                if whole <= KEPT_WHOLE * view.size.0 * view.size.1 { (self.reduced(&level, reduce)?, 1.0) } else { (level, reduce) }
            }
            false => (base, 1.0),
        };
        let request = Request { scale: reduce, ..request };
        let inputs = Inputs { patches: patches_for(&self.patches, settings), custom_flat_field: custom_for(&self.custom, settings) };
        let image = self.pipeline.image(&mut self.graph, &source, settings, &request, &inputs).map_err(|e| e.to_string())?;
        let image = self.proof.laid(&mut self.graph, image, settings)?;
        let extent = image.extent();
        let stretch = scale * full.0 / extent.w as f64;
        let (vx, vy, vw, vh) = placement.visible;
        // One size a view, the visible part's with a margin each side, so a pan keeps every texture it renders
        // into and the picture it moves at once shows no bare edge before the next render lands.
        let span = |v: f64, len: f64, origin: i32, end: i32| {
            let margin = (len / stretch * PAN_MARGIN).ceil() as i32;
            let size = ((len / stretch).ceil() as i32 + 1 + 2 * margin).min(end - origin);
            let start = (origin + (v / stretch).floor() as i32 - margin).clamp(origin, end - size);
            (start, start + size)
        };
        let ((x0, x1), (y0, y1)) = (span(vx, vw, extent.x, extent.right()), span(vy, vh, extent.y, extent.bottom()));
        let region = Rect::from_corners(x0, y0, x1, y1);
        if region.is_empty() {
            return Err(tr("the image is off the canvas").into());
        }
        let placed = Placed {
            x: placement.image.0 + f64::from(region.x - extent.x) * stretch,
            y: placement.image.1 + f64::from(region.y - extent.y) * stretch,
            width: f64::from(region.w) * stretch,
            height: f64::from(region.h) * stretch,
            full,
            whole: region == extent,
            at_full_resolution: view.full.is_some(),
        };
        Ok((self.one_texture(&image, Some(region))?, Some(placed)))
    }

    pub(super) fn one_texture(&mut self, image: &Image, region: Option<Rect>) -> Result<Source, String> {
        let region = region.unwrap_or_else(|| image.extent());
        let source = self.graph.materialise_cached(image, region, Some(&mut self.cache)).map_err(|e| e.to_string())?;
        if source.view().is_none() {
            return Err(tr("the render is wider than one texture").into());
        }
        Ok(source)
    }

    /// The smallest halving of `base` still at or past `side` on its long side, and the factor left to
    /// reach `side` from it, under two: a zoom's every step reads a level near the screen's size.
    pub(super) fn level(&mut self, base: &Source, side: usize, long: usize) -> Result<(Source, f64), String> {
        let id = base.id();
        let at = match self.fitted.halves.iter().position(|(k, _)| *k == id) {
            Some(at) => at,
            None => {
                if self.fitted.halves.len() >= BASES {
                    self.fitted.halves.remove(0);
                }
                self.fitted.halves.push((id, Vec::new()));
                self.fitted.halves.len() - 1
            }
        };
        let (mut from, mut from_long) = (base.clone(), long);
        for level in 0.. {
            if from_long / 2 < side.max(MIN_HALF) {
                break;
            }
            if self.fitted.halves[at].1.len() == level {
                let half = self.graph.materialise_half(&Image::new(&from).lanczos(0.5)).map_err(|e| e.to_string())?;
                self.fitted.halves[at].1.push(half);
            }
            from = self.fitted.halves[at].1[level].clone();
            let e = from.extent();
            from_long = e.w.max(e.h) as usize;
        }
        Ok((from, side as f64 / from_long as f64))
    }

    /// The whole picture `COUNTED_SIDE` long from the preview, for the histogram, which so reads the same
    /// pixels at fit and zoomed; `None` while the settings and the preview are those last counted.
    pub fn counted(&mut self, negative: &Negative, settings: &PipelineSettings) -> Result<Option<Source>, String> {
        let key = (settings.clone(), negative.preview.id());
        if self.fitted.counted.as_ref() == Some(&key) {
            return Ok(None);
        }
        let (w, h) = negative.size();
        let scale = (COUNTED_SIDE / w.max(h) as f64).min(1.0);
        let request = Request { scale, full_width: Some(negative.full_width), ..Request::default() };
        self.read_custom(settings);
        let inputs = Inputs { patches: patches_for(&self.patches, settings), custom_flat_field: custom_for(&self.custom, settings) };
        let image = self.pipeline.image(&mut self.graph, &negative.preview, settings, &request, &inputs).map_err(|e| e.to_string())?;
        let image = self.proof.laid(&mut self.graph, image, settings)?;
        let source = self.graph.materialise(&image).map_err(|e| e.to_string())?;
        self.fitted.counted = Some(key);
        Ok(Some(source))
    }

    /// `level` reduced by `reduce` whole, the last one kept.
    fn reduced(&mut self, level: &Source, reduce: f64) -> Result<Source, String> {
        let key = (level.id(), reduce.to_bits());
        if let Some((k, source)) = &self.fitted.kept
            && *k == key
        {
            return Ok(source.clone());
        }
        let source = self.graph.materialise(&Image::new(level).lanczos(reduce)).map_err(|e| e.to_string())?;
        self.fitted.kept = Some((key, source.clone()));
        Ok(source)
    }
}

/// The share of the view rendered past each of its sides, which a pan reveals before its render lands.
const PAN_MARGIN: f64 = 0.125;
/// The pixels a view rendered during a gesture holds at most, about what a 1440p window's view does.
const GESTURE_PIXELS: f64 = 4.0e6;
/// The long side of the picture the histogram counts.
const COUNTED_SIDE: f64 = 1024.0;
/// A reduction kept whole when it holds at most this many canvases of pixels.
const KEPT_WHOLE: f64 = 2.0;

/// The smallest halving kept: under it a reduction costs little from the level above.
const MIN_HALF: usize = 64;
/// The bases whose halvings are kept: the preview and the full frame, so neither evicts the other.
const BASES: usize = 2;

/// The halvings of the bases the canvas reads, and the last whole reduction made from one.
#[derive(Default)]
pub(crate) struct Fitting {
    halves: Vec<(u64, Vec<Source>)>,
    kept: Option<((u64, u64), Source)>,
    /// The settings and preview the histogram's picture was last made from.
    counted: Option<(PipelineSettings, u64)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_graph::Rect;

    #[test]
    fn a_reduction_through_the_halvings_is_the_direct_one() {
        let Ok(device) = crate::render::Device::open() else { return };
        let mut engine = Engine::on(&device);
        let (w, h) = (1600usize, 1000usize);
        let wave = |x: f32, y: f32| [0.4 + 0.3 * (x * 0.05).sin() * (y * 0.031).cos(), 0.3 + 0.2 * x / w as f32, 0.5, 1.0];
        let pixels: Vec<[f32; 4]> = (0..w * h).map(|i| wave((i % w) as f32, (i / w) as f32)).collect();
        let full = engine.graph.upload(w as u32, h as u32, &pixels).unwrap();
        let (level, reduce) = engine.level(&full, 300, w).unwrap();
        assert!(level.extent().w < w as u32 && reduce > 0.5, "{:?} {reduce}", level.extent());
        let through = Image::new(&level).lanczos(reduce);
        let read = |e: &mut Engine, image: &Image| e.graph.render(image, image.extent(), None).unwrap();
        let direct = Image::new(&full).lanczos(300.0 / w as f64);
        assert_eq!(through.extent(), direct.extent());
        let (ours, theirs) = (read(&mut engine, &through), read(&mut engine, &direct));
        let gap = |a: &[[f32; 4]], b: &[[f32; 4]]| a.iter().zip(b).map(|(p, q)| (0..3).map(|c| (p[c] - q[c]).abs()).sum::<f32>()).sum::<f32>() / a.len() as f32;
        assert!(gap(&ours, &theirs) < 2e-3, "{}", gap(&ours, &theirs));
        // Twin: the frame a few pixels over reduces to another picture, which the gap sees.
        let moved = Image::new(&full).then(oni_graph::stages::Crop(Rect::new(6, 0, w as u32 - 6, h as u32))).lanczos(300.0 / w as f64);
        let other = read(&mut engine, &moved);
        assert!(gap(&ours[..other.len().min(ours.len())], &other) > 1e-2);
    }
}
