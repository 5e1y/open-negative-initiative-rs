//! Many small renders laid on one sheet, as Edit Together's grid asks, kept on the GPU for the canvas.

use super::*;

impl Engine {
    /// Each negative on its own settings at `side`, laid at the place `place` gives its size on one clear
    /// sheet of `size`, which stays on the GPU.
    pub fn render_laid(
        &mut self,
        tiles: &[(&Negative, &PipelineSettings)],
        side: usize,
        place: &dyn Fn(usize, (u32, u32)) -> (i32, i32),
        size: (u32, u32),
    ) -> Result<Source, String> {
        let mut images = Vec::with_capacity(tiles.len());
        for (negative, settings) in tiles {
            let (w, h) = negative.size();
            let request = Request { scale: (side as f64 / w.max(h) as f64).min(1.0), full_width: Some(negative.full_width), ..Request::default() };
            self.read_custom(settings);
            let inputs = Inputs { patches: patches_for(&self.patches, settings), custom_flat_field: custom_for(&self.custom, settings) };
            let image = self.pipeline.image(&mut self.graph, &negative.preview, settings, &request, &inputs).map_err(|e| e.to_string())?;
            images.push(self.proof.laid(&mut self.graph, image, settings)?);
        }
        let placed: Vec<(&Image, (i32, i32))> = images.iter().enumerate().map(|(i, image)| (image, place(i, (image.extent().w, image.extent().h)))).collect();
        self.graph.laid_out(&placed, size).map_err(|e| e.to_string())
    }

}

impl Engine {
    /// A frame's preview reduced to `side` on its longest side, laid as a source of its own: a tile
    /// from what the editor already holds, with no decode.
    pub fn tile_of(&mut self, negative: &Negative, side: usize) -> Result<Negative, String> {
        let (w, h) = negative.size();
        let long = w.max(h) as f64;
        let mut image = Image::new(&negative.preview);
        if long > side as f64 {
            image = image.lanczos(side as f64 / long);
        }
        let preview = self.graph.materialise_half(&image).map_err(|e| e.to_string())?;
        Ok(Negative { preview, full_width: negative.full_width, undeclared: negative.undeclared })
    }
}
