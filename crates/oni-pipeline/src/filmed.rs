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
}
