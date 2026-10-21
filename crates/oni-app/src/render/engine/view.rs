//! The canvas's render: the region the view shows, at the scale it shows it, kept as one texture.

use super::*;

impl Engine {
    /// `Pipeline.apply` left on the GPU in one texture for the display kernel to read. With a view,
    /// only its visible region, from the preview or past its ratio from the full frame, each reduced
    /// once to the scale shown, so a notch computes no hidden pixel.
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
        let at_full = crate::viewport::reads_full(view.preview, view.settled && vp.needs_full_resolution(ratio, full, view.size, crate::viewport::NOMINAL_BUDGET));
        let (base, base_ratio) = match (&view.full, at_full) {
            (Some(f), true) => (f.clone(), 1.0),
            _ => (negative.preview.clone(), ratio),
        };
        let e = base.extent();
        let long = e.w.max(e.h) as usize;
        let side = ((long as f64 * (scale / base_ratio).min(1.0)).round() as usize).max(1);
        let source = if side < long { self.fitted(&base, side, long)? } else { base };
        let inputs = Inputs { patches: patches_for(&self.patches, settings), custom_flat_field: custom_for(&self.custom, settings) };
        let image = self.pipeline.image(&mut self.graph, &source, settings, &request, &inputs).map_err(|e| e.to_string())?;
        let image = self.proof.laid(&mut self.graph, image, settings)?;
        let extent = image.extent();
        let stretch = scale * full.0 / extent.w as f64;
        let (vx, vy, vw, vh) = placement.visible;
        let corner = |v: f64, origin: i32, f: fn(f64) -> f64| origin + f(v / stretch) as i32;
        let region = Rect::from_corners(
            corner(vx, extent.x, f64::floor).max(extent.x),
            corner(vy, extent.y, f64::floor).max(extent.y),
            corner(vx + vw, extent.x, f64::ceil).min(extent.right()),
            corner(vy + vh, extent.y, f64::ceil).min(extent.bottom()),
        );
        if region.is_empty() {
            return Err("the image is off the canvas".into());
        }
        let placed = Placed {
            x: placement.image.0 + f64::from(region.x - extent.x) * stretch,
            y: placement.image.1 + f64::from(region.y - extent.y) * stretch,
            width: f64::from(region.w) * stretch,
            height: f64::from(region.h) * stretch,
            full,
            whole: region == extent,
            at_full_resolution: at_full && view.full.is_some(),
        };
        Ok((self.one_texture(&image, Some(region))?, Some(placed)))
    }

    pub(super) fn one_texture(&mut self, image: &Image, region: Option<Rect>) -> Result<Source, String> {
        let region = region.unwrap_or_else(|| image.extent());
        let source = self.graph.materialise_cached(image, region, Some(&mut self.cache)).map_err(|e| e.to_string())?;
        if source.view().is_none() {
            return Err("the render is wider than one texture".into());
        }
        Ok(source)
    }

    /// `base` reduced to `side` on its long side, from the smallest of its halvings still past it, so
    /// each step of a zoom reduces a few megapixels rather than the whole frame.
    pub(super) fn fitted(&mut self, base: &Source, side: usize, long: usize) -> Result<Source, String> {
        let id = base.id();
        if let Some((kept, s, source)) = &self.fitted.kept
            && (*kept, *s) == (id, side)
        {
            return Ok(source.clone());
        }
        self.fitted.kept = None;
        if self.fitted.halves.as_ref().is_none_or(|(k, _)| *k != id) {
            self.fitted.halves = Some((id, Vec::new()));
        }
        let (mut from, mut from_long) = (base.clone(), long);
        for level in 0.. {
            if from_long / 2 < side.max(MIN_HALF) {
                break;
            }
            let halves = &mut self.fitted.halves.as_mut().expect("set above").1;
            if halves.len() == level {
                let half = self.graph.materialise(&Image::new(&from).lanczos(0.5)).map_err(|e| e.to_string())?;
                halves.push(half);
            }
            from = halves[level].clone();
            let e = from.extent();
            from_long = e.w.max(e.h) as usize;
        }
        let reduced = Image::new(&from).lanczos(side as f64 / from_long as f64);
        let source = self.graph.materialise(&reduced).map_err(|e| e.to_string())?;
        self.fitted.kept = Some((id, side, source.clone()));
        Ok(source)
    }
}

/// The smallest halving kept: under it a reduction costs little from the level above.
const MIN_HALF: usize = 512;

/// The last reduction made and the halvings of the base it came from.
#[derive(Default)]
pub(crate) struct Fitting {
    halves: Option<(u64, Vec<Source>)>,
    kept: Option<(u64, usize, Source)>,
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
        let through = engine.fitted(&full, 300, w).unwrap();
        let read = |e: &mut Engine, image: &Image| e.graph.render(image, image.extent(), None).unwrap();
        let direct = Image::new(&full).lanczos(300.0 / w as f64);
        assert_eq!(through.extent(), direct.extent());
        let (ours, theirs) = (read(&mut engine, &Image::new(&through)), read(&mut engine, &direct));
        let gap = |a: &[[f32; 4]], b: &[[f32; 4]]| a.iter().zip(b).map(|(p, q)| (0..3).map(|c| (p[c] - q[c]).abs()).sum::<f32>()).sum::<f32>() / a.len() as f32;
        assert!(gap(&ours, &theirs) < 2e-3, "{}", gap(&ours, &theirs));
        // Twin: the frame a few pixels over reduces to another picture, which the gap sees.
        let moved = Image::new(&full).then(oni_graph::stages::Crop(Rect::new(6, 0, w as u32 - 6, h as u32))).lanczos(300.0 / w as f64);
        let other = read(&mut engine, &moved);
        assert!(gap(&ours[..other.len().min(ours.len())], &other) > 1e-2);
    }
}
