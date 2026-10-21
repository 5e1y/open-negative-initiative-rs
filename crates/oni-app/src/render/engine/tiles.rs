//! Many small renders at once, as Edit Together's grid asks: each kept on the GPU, all read back together.

use super::*;

impl Engine {
    /// Each negative on its own settings at `side`, every one evaluated before any is read back.
    pub fn render_tiles(&mut self, tiles: &[(&Negative, &PipelineSettings)], side: usize) -> Vec<Result<Linear, String>> {
        let kept: Vec<Result<Source, String>> = tiles
            .iter()
            .map(|(negative, settings)| {
                let (w, h) = negative.size();
                let request = Request { scale: (side as f64 / w.max(h) as f64).min(1.0), full_width: Some(negative.full_width), ..Request::default() };
                self.read_custom(settings);
                let inputs = Inputs { patches: patches_for(&self.patches, settings), custom_flat_field: custom_for(&self.custom, settings) };
                let image = self.pipeline.image(&mut self.graph, &negative.preview, settings, &request, &inputs).map_err(|e| e.to_string())?;
                let image = self.proof.laid(&mut self.graph, image, settings)?;
                self.graph.materialise(&image).map_err(|e| e.to_string())
            })
            .collect();
        let sources: Vec<&Source> = kept.iter().flatten().collect();
        let mut read = self.graph.read_back(&sources).into_iter();
        kept.iter()
            .map(|k| {
                let source = k.as_ref().map_err(Clone::clone)?;
                let extent = source.extent();
                Ok(Linear { width: extent.w as usize, height: extent.h as usize, pixels: read.next().unwrap_or_default() })
            })
            .collect()
    }
}
