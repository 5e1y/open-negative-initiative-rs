//! `Pipeline.filmed`: a digital photo's scene on its virtual film, the halation's glow read off the film's plate.

use super::*;

impl Pipeline {
    /// A digital photo's scene on its virtual film: exposed, developed and, a colour negative, scanned through its dyes
    /// as a real negative is; a slide's layers as they are, for the projector; the neutral film's code as it is. On a
    /// scan of extent `effects` the film's layers take its blur and its halation between exposure and development.
    pub fn filmed(&mut self, graph: &mut Graph, image: Image, s: &PipelineSettings, effects: Option<Rect>) -> Result<Image> {
        let plate = (image.clone(), IDENTITY);
        self.filmed_on(graph, image, s, effects, &plate)
    }

    /// `filmed`, the halation's glow drawn on `plate`, the scene's whole frame lit as `image` is, where `plate.1` takes a
    /// pixel's centre of `image`: drawn once a photo and its light, read at every scale and place.
    pub fn filmed_on(&mut self, graph: &mut Graph, image: Image, s: &PipelineSettings, effects: Option<Rect>, plate: &(Image, [[f32; 3]; 2])) -> Result<Image> {
        let Some(f) = s.film() else { return Ok(image) };
        // The neutral film's layers are the scene's own channels, their exposure its light.
        if let Some(k) = film::neutral::kernel(f.stock) {
            let glow = self.glow(graph, s, effects, plate.0.clone(), plate.1)?;
            let scene = effects.and_then(|scan| layers::laid(image.clone(), s, scan, glow.as_ref())).unwrap_or(image);
            return Ok(scene.then(stages::neutral_negative(&film::neutral::exposed(k, f.exposure))));
        }
        let stock = film::stock(f.stock);
        let key = (f.stock, stock.revision);
        if let Entry::Vacant(e) = self.films.entry(key) {
            let (w, h) = stock.kernel.table_size();
            let scan = stock.scan.as_ref().map(|scan| graph.upload(scan.projection.bands as u32, 2, &scan.table)).transpose()?;
            e.insert((graph.upload(w as u32, h as u32, &stock.table)?, scan));
        }
        let (table, scan) = self.films[&key].clone();
        let (k, table) = (stock.kernel_at(f.exposure), [Image::new(&table)]);
        let glow = self.glow(graph, s, effects, plate.0.clone().then_reading(stages::virtual_exposure(&k), &table), plate.1)?;
        let on_layers = |exposed: Image| effects.and_then(|scan| layers::laid(exposed, s, scan, glow.as_ref()));
        // No effect on its layers: the exposure and the development are one pass.
        let developed = match on_layers(image.clone().then_reading(stages::virtual_exposure(&k), &table)) {
            Some(laid) => laid.then_reading(stages::virtual_development(&k), &table),
            None => image.then_reading(stages::virtual_negative(&k), &table),
        };
        Ok(match (&stock.scan, scan) {
            (Some(k), Some(table)) => developed.then_reading(stages::projection(&k.projection), &[Image::new(&table)]),
            _ => developed,
        })
    }

    /// The glow `s`'s halation reads off the plate `exposed`, a film's layers' exposures, through `map`: drawn when its
    /// recipe changes, kept while a zoom or a pan reads it again; none at rest or with no effect.
    pub fn glow(&mut self, graph: &mut Graph, s: &PipelineSettings, effects: Option<Rect>, exposed: Image, map: [[f32; 3]; 2]) -> Result<Option<halation::Glow>> {
        if effects.is_none() || !halation::lays(s) {
            return Ok(None);
        }
        let image = halation::glow(exposed, s);
        let key = image.key();
        let plate = match &self.glow {
            Some((k, plate)) if *k == key => plate.clone(),
            _ => {
                let plate = graph.materialise(&image)?;
                self.glow = Some((key, plate.clone()));
                plate
            }
        };
        Ok(Some(halation::Glow { plate, map }))
    }

    /// The plate of the grain `k` on a scan of `full` pixels, drawn over `base` when its photo, its grains' size or the
    /// scan's size change, and kept while a zoom, a pan or a notch reads it again.
    pub fn grain_plate(&mut self, graph: &mut Graph, k: &oni_render::Grain, full: (f64, f64), base: &Image) -> Result<grain::Plate> {
        let key = (k.seed, k.radius.to_bits(), k.silver, (full.0.to_bits(), full.1.to_bits()));
        if let Some((kept, plate)) = &self.grain_plate
            && *kept == key
        {
            return Ok(plate.clone());
        }
        // The finest level drawn, each next the mean of its 2 × 2 texels: a pixel reads the mean of the grain under it,
        // as a reduced render reads the full grain averaged.
        let pitches = grain::pitches(k.radius, full);
        let mut levels = vec![(graph.materialise(&grain::level(base, k.radius, pitches[0], full, k.seed, k.silver))?, pitches[0])];
        for &pitch in &pitches[1..] {
            let half = graph.materialise(&Image::new(&levels[levels.len() - 1].0).then(stages::GrainHalf))?;
            levels.push((half, pitch));
        }
        // The gain every half octave of footprint, so the plate lays the grain a render drawn at that footprint did:
        // the deviation of the field drawn there, its spread included, over the plate's read as such a render reads it,
        // on a patch about the frame's centre.
        let deviation = |values: &[f32]| {
            let mean = values.iter().sum::<f32>() / values.len() as f32;
            (values.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / values.len() as f32).sqrt()
        };
        let channels = if k.silver { 1 } else { 3 };
        let pitches: Vec<f32> = levels.iter().map(|l| l.1).collect();
        let long = full.0.max(full.1) as f32;
        let mut gains = Vec::new();
        for footprint in std::iter::successors(Some(1.0f32), |f| Some(f * std::f32::consts::SQRT_2)).take_while(|f| long / f >= 2.0 * GAIN_PATCH as f32) {
            let centre = (0.5 * full.0 as f32, 0.5 * full.1 as f32);
            let first = (centre.0 - 0.5 * GAIN_PATCH as f32 * footprint, centre.1 - 0.5 * GAIN_PATCH as f32 * footprint);
            // The field drawn at that footprint, a texel a render's pixel.
            let at = Rect::new((first.0 / footprint) as i32, (first.1 / footprint) as i32, GAIN_PATCH, GAIN_PATCH);
            let drawn = graph.render(&grain::level(base, k.radius, footprint, full, k.seed, k.silver), at, None)?;
            let drawn: Vec<f32> = drawn.iter().flat_map(|p| p[..channels].to_vec()).collect();
            // The plate read there, as the laying reads it.
            let (level, half) = grain::reading(&pitches, footprint);
            let (source, pitch) = &levels[level];
            let reach = (half.max(1.0) + 2.0).ceil() as i32;
            let span = |a: f32| ((a / pitch).floor() as i32 - reach, ((a + GAIN_PATCH as f32 * footprint) / pitch).ceil() as i32 + reach);
            let ((x0, x1), (y0, y1)) = (span(first.0), span(first.1));
            let texel_rect = Rect::from_corners(x0, y0, x1, y1).intersect(&source.extent());
            let texels = oni_render::cpu::Grid::new(texel_rect.x, texel_rect.y, texel_rect.w as usize, texel_rect.h as usize, graph.render(&Image::new(source), texel_rect, None)?);
            let onto = [[1.0 / pitch, 0.0, 0.0], [0.0, 1.0 / pitch, 0.0]];
            let read: Vec<f32> = (0..GAIN_PATCH * GAIN_PATCH)
                .flat_map(|i| {
                    let q = [first.0 + ((i % GAIN_PATCH) as f32 + 0.37) * footprint, first.1 + ((i / GAIN_PATCH) as f32 + 0.37) * footprint];
                    oni_render::cpu::plate_read(&texels, onto, q, half)[..channels].to_vec()
                })
                .collect();
            let spread = oni_render::Grain::level(k.radius, footprint).spread;
            gains.push((footprint, spread * deviation(&drawn) / deviation(&read).max(f32::MIN_POSITIVE)));
        }
        if gains.is_empty() {
            gains.push((1.0, 1.0));
        }
        let plate = grain::Plate { levels, gains };
        self.grain_plate = Some((key, plate.clone()));
        Ok(plate)
    }
}

/// The side of the patch a plate's gain is measured on, in a render's pixels.
const GAIN_PATCH: u32 = 128;
