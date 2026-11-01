//! Edit Together's drawing thread: the latest grid asked, each tile rendered on its own settings and
//! laid in rows on one sheet; and a frame decoded into a tile's source.

use super::*;
use crate::latest::Taken;

/// A frame decoded at half size, reduced so its crop spans a tile and laid on the GPU, unless the group
/// `id` was left meanwhile; a RAW's half decode is the editor's, kept on disk, which the balance's copy reads.
pub(super) fn tile_source(engine: &Shared, path: &str, id: u64) -> Result<Arc<Negative>, String> {
    let mosaic = oni_decode::decodes_via_mosaic(Path::new(path));
    let settings = oni_settings::sidecar::read(Path::new(path), mosaic, true).map_err(|e| e.to_string())?.settings;
    tile_decoded(engine, path, &settings, || GROUPS.load(Ordering::Relaxed) == id).map(Arc::new)
}

/// A frame on `settings` decoded at half size and reduced so its crop spans a tile, laid on the GPU
/// while `wanted` still holds once the decode is done.
pub(super) fn tile_decoded(engine: &Shared, path: &str, settings: &PipelineSettings, wanted: impl Fn() -> bool) -> Result<Negative, String> {
    let mut decoded = render::decode_for(Path::new(path), render::Reading::of(settings), oni_decode::HALF)?;
    if !wanted() {
        return Err(String::new());
    }
    let side = source_side(settings, (decoded.linear.width, decoded.linear.height));
    decoded.linear = reduced(decoded.linear, side);
    engine.prepare(decoded)
}

/// The frame's settings as its sidecar holds them: whether it reads as a slide, the key its kept
/// preview is found by, and its crop.
pub(super) fn settings_of(path: &str) -> Option<PipelineSettings> {
    let mosaic = oni_decode::decodes_via_mosaic(Path::new(path));
    oni_settings::sidecar::read(Path::new(path), mosaic, true).ok().map(|l| l.settings)
}

/// A tile's longest side drawn at `scale`.
fn side(scale: f64) -> usize {
    ((TILE as f64 * scale).round() as usize).max(1)
}

/// The drawing thread: the latest grid asked, each tile rendered on its own settings and laid in rows.
pub(super) fn draw_grids(engine: Shared) {
    loop {
        let Taken::Value(ask) = SLOT.take(None) else { return };
        let started = Instant::now();
        let cells: Vec<usize> = ask.tiles.iter().enumerate().filter_map(|(i, t)| t.as_ref().map(|_| i)).collect();
        let present: Vec<Laid> = ask
            .tiles
            .iter()
            .flatten()
            .map(|t| match t {
                Tile::Frame(n, s) => Laid::Frame(n.as_ref(), s),
                Tile::StandIn(picture) => Laid::Shown(picture),
            })
            .collect();
        let (scale, sheet) = (ask.scale, &ask.sheet);
        let (wx, wy, ww, wh) = ask.window;
        let size = (ww, wh);
        let Ok(mut guard) = engine.lock() else { continue };
        let at = |i: usize, tile| {
            let (x, y) = sheet.place(cells[i], tile, scale);
            (x - wx as i32, y - wy as i32)
        };
        let Ok(drawn) = guard.render_laid(&present, side(scale), &at, size) else { continue };
        // The histogram's sheet: every tile at one scale, so a zoom never moves what it counts.
        let counted = ask.counted.as_ref().and_then(|all| {
            let cells: Vec<usize> = all.iter().enumerate().filter_map(|(i, t)| t.as_ref().map(|_| i)).collect();
            let present: Vec<Laid> = all.iter().flatten().map(|(n, s)| Laid::Frame(n.as_ref(), s)).collect();
            let place = |i: usize, tile| sheet.place(cells[i], tile, COUNTED_SCALE);
            guard.render_laid(&present, side(COUNTED_SCALE), &place, sheet.drawn(COUNTED_SCALE)).ok()
        });
        // The gaps are clear, so the screen's encoding lays the sheet's black under them.
        drop(guard);
        let (group, revision, at, pipeline, laid) = (ask.group, ask.revision, ask.at, started.elapsed(), (ask.window, ask.scale));
        with_shell(move |c| c.grid_landed(group, revision, at, drawn, pipeline, laid, counted));
    }
}
