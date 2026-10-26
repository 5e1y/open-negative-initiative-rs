//! The export's renders: the frame decoded whole, rendered unproofed at full resolution and read back
//! in bands, the estimate's reduced sample, and one frame written.

use super::{Target, BAND_ROWS};
use crate::render::{Decoded, Engine, Negative, Shared};
use oni_export::{ExifMetadata, Failure, Image, SAMPLE_SIDE, Settings};
use oni_graph::Image as Recipe;
use oni_pipeline::Request;
use oni_settings::PipelineSettings;
use std::path::Path;

/// `RawDecode.linear(source)` at full resolution, the export's own decode.
pub fn full_decode(path: &Path, reading: crate::render::Reading) -> Result<Decoded, Failure> {
    crate::render::decode_full(path, reading).map_err(|_| Failure::Decode)
}

/// `Pipeline.apply` at full resolution and unproofed, with the canvas's masks and patches, read
/// back in bands of rows; the decode held in half floats, as the canvas holds it.
pub fn render_full(engine: &mut Engine, decoded: Decoded, settings: &PipelineSettings) -> Result<Image, Failure> {
    let l = decoded.linear;
    let source = engine.parts().0.upload_half(l.width as u32, l.height as u32, &l.pixels).map_err(|_| Failure::Render)?;
    drop(l.pixels);
    rendered(engine, source, settings)
}

/// `render_full` with the decode laid on the GPU beside the engine, so another frame renders meanwhile.
pub fn render_full_beside(engine: &Shared, decoded: Decoded, settings: &PipelineSettings) -> Result<Image, Failure> {
    let source = engine.upload(&decoded.linear).map_err(|_| Failure::Render)?;
    drop(decoded);
    let mut engine = engine.lock().map_err(|_| Failure::Render)?;
    rendered(&mut engine, source, settings)
}

fn rendered(engine: &mut Engine, source: oni_graph::Source, settings: &PipelineSettings) -> Result<Image, Failure> {
    let image = engine.unproofed(&source, settings, &Request::default()).map_err(|_| Failure::Render)?;
    let (graph, _) = engine.parts();
    let out = read_back(graph, &image);
    drop(source);
    graph.purge();
    out
}

fn read_back(graph: &mut oni_graph::Graph, image: &Recipe) -> Result<Image, Failure> {
    let e = image.extent();
    let mut pixels = Vec::with_capacity(e.area() as usize * 4);
    for top in (0..e.h).step_by(BAND_ROWS as usize) {
        let band = oni_graph::Rect::new(e.x, e.y + top as i32, e.w, BAND_ROWS.min(e.h - top));
        let rows = graph.render(image, band, None).map_err(|_| Failure::Render)?;
        pixels.extend(rows.into_iter().flatten());
    }
    Ok(Image { width: e.w as usize, height: e.h as usize, pixels })
}

/// `Pipeline.apply` on a preview reduced to `side`, unproofed: the estimate's sample.
pub fn render_reduced(engine: &mut Engine, negative: &Negative, settings: &PipelineSettings, side: usize) -> Result<Image, Failure> {
    let (w, h) = negative.size();
    let scale = (side as f64 / w.max(h) as f64).min(1.0);
    let request = Request { scale, full_width: Some(negative.full_width), ..Request::default() };
    let image = engine.unproofed(&negative.preview, settings, &request).map_err(|_| Failure::Render)?;
    read_back(engine.parts().0, &image)
}

/// The 500 px sample `refreshEstimate` weighs: the frame's own, whatever the export's settings.
pub fn sample(engine: &Shared, negative: &Negative, settings: &PipelineSettings) -> Option<Image> {
    render_reduced(&mut *engine.lock().ok()?, negative, settings, SAMPLE_SIDE as usize).ok()
}

/// One frame written: its full render, its file, then its line kept in its sidecar; a batch writes the
/// same bytes through `batch::write_batch`.
pub fn write_one(engine: &Shared, target: &Target, to: oni_export::Landing<'_>, export: &Settings, metadata: &ExifMetadata) -> Result<(), Failure> {
    let render = || {
        let decoded = full_decode(&target.path, crate::render::Reading::of(&target.settings))?;
        let mut engine = engine.lock().map_err(|_| Failure::Render)?;
        render_full(&mut engine, decoded, &target.settings)
    };
    let date = (!metadata.date.is_empty()).then_some(metadata.date.as_str());
    oni_export::write(&target.path, to, render, &target.settings.paper_correction, export, metadata, date)?;
    let mosaic = oni_decode::decodes_via_mosaic(&target.path);
    let _ = oni_settings::sidecar::set_exif(metadata.clone(), &target.path, mosaic);
    Ok(())
}

