//! The export's renders: the frame decoded whole, rendered unproofed at full resolution and read back
//! in bands, the estimate's sample, and one frame written.

use super::{Target, BAND_ROWS};
use crate::render::{Decoded, Engine, Negative, Shared};
use oni_export::{ExifMetadata, Failure, Image, Settings};
use oni_graph::Image as Recipe;
use oni_pipeline::Request;
use oni_settings::PipelineSettings;
use std::path::Path;

/// `RawDecode.linear(source)` at full resolution, the export's own decode.
pub fn full_decode(path: &Path, reading: crate::render::Reading) -> Result<Decoded, Failure> {
    crate::render::decode_full(path, reading).map_err(|_| Failure::Decode)
}

/// `Pipeline.apply` at full resolution and unproofed on an engine already held, with the masks and the
/// patches on disk alone, read back in bands of rows; the decode held in half floats, as the canvas holds it.
#[cfg(test)]
pub(crate) fn render_full(engine: &mut Engine, decoded: Decoded, settings: &PipelineSettings) -> Result<Image, Failure> {
    let l = decoded.linear;
    let source = engine.parts().0.upload_half(l.width as u32, l.height as u32, &l.pixels).map_err(|_| Failure::Render)?;
    drop(l.pixels);
    rendered(engine, source, settings)
}

/// `Pipeline.apply` at full resolution and unproofed, with the canvas's masks and every stroke's patch,
/// made first when not on disk, read back in bands of rows; the decode laid beside the engine, so
/// another frame renders meanwhile.
pub fn render_full_beside(engine: &Shared, decoded: Decoded, settings: &PipelineSettings) -> Result<Image, Failure> {
    let source = engine.upload(&decoded.linear).map_err(|_| Failure::Render)?;
    drop(decoded);
    heal(engine, &source, settings)?;
    let mut engine = engine.lock().map_err(|_| Failure::Render)?;
    rendered(&mut engine, source, settings)
}

/// Every stroke of `settings` given its patch on `full`, the frame's whole decode: a render lays only
/// the patches on disk, and one that cannot be made fails the file rather than leave its speck.
pub fn heal(engine: &Shared, full: &oni_graph::Source, settings: &PipelineSettings) -> Result<(), Failure> {
    let Some(caches) = engine.lock().map_err(|_| Failure::Render)?.caches.clone() else { return Ok(()) };
    let (mut made, mut failed) = (0, None);
    crate::heal::ensure(engine, full, &caches, &mut Default::default(), || Some(settings.clone()), |one| match one {
        Ok(_) => made += 1,
        Err((_, e)) => failed = Some(e),
    });
    if made > 0 {
        crate::timing::line(&format!("export: {made} dust patches generated"));
    }
    match failed {
        Some(e) => {
            crate::timing::line(&format!("export: a dust patch not generated: {e}"));
            Err(Failure::Dust)
        }
        None => Ok(()),
    }
}

/// `heal` for a render that reads a reduced decode, a contact sheet's: the frame decoded whole only
/// when one of its strokes has no patch on disk.
pub fn heal_ahead(engine: &Shared, target: &Target) -> Result<(), Failure> {
    let Some(caches) = engine.lock().map_err(|_| Failure::Render)?.caches.clone() else { return Ok(()) };
    if crate::heal::pending(&caches, &target.settings, &Default::default()).is_empty() {
        return Ok(());
    }
    let decoded = full_decode(&target.path, crate::render::Reading::of(&target.settings))?;
    let full = engine.upload(&decoded.linear).map_err(|_| Failure::Render)?;
    drop(decoded);
    heal(engine, &full, &target.settings)
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

/// A sheet laid on the GPU read back in bands of rows, then let go.
pub fn read_laid(engine: &mut Engine, sheet: oni_graph::Source) -> Result<Image, Failure> {
    let (graph, _) = engine.parts();
    let out = read_back(graph, &Recipe::new(&sheet));
    drop(sheet);
    graph.purge();
    out
}

/// The sample the estimate weighs: `Pipeline.apply` unproofed on the editor's reduced decode at `scale`
/// of the full frame, the scale the file is written at, never past what that decode holds.
pub fn sample(engine: &Shared, negative: &Negative, settings: &PipelineSettings, scale: f64) -> Option<Image> {
    let (w, _) = negative.size();
    let request = Request { scale: (scale * negative.full_width / f64::from(w.max(1))).min(1.0), full_width: Some(negative.full_width), ..Request::default() };
    let mut engine = engine.lock().ok()?;
    let image = engine.unproofed(&negative.preview, settings, &request).ok()?;
    read_back(engine.parts().0, &image).ok()
}

/// One frame written: its full render, its file, then what a hand set of its line kept in its sidecar;
/// a batch writes the same bytes through `batch::write_batch`.
pub fn write_one(engine: &Shared, target: &Target, to: oni_export::Landing<'_>, export: &Settings, metadata: &ExifMetadata, typed: &oni_settings::ExifLine) -> Result<(), Failure> {
    let render = || {
        let decoded = full_decode(&target.path, crate::render::Reading::of(&target.settings))?;
        render_full_beside(engine, decoded, &target.settings)
    };
    oni_export::write(&target.path, to, render, &target.settings.paper_correction, export, metadata)?;
    super::keep_line(&target.path, typed);
    Ok(())
}

