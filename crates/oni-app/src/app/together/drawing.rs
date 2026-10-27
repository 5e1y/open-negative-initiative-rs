//! Edit Together's drawing thread: the latest grid asked, each tile rendered on its own settings and
//! laid in rows on one sheet; and a frame decoded into a tile's source.

use super::*;
use crate::latest::Taken;

/// A frame decoded at half size when it can, reduced to a tile and laid on the GPU, unless the group
/// `id` was left meanwhile.
pub(super) fn tile_source(engine: &Shared, path: &str, id: u64) -> Result<Arc<Negative>, String> {
    let mosaic = oni_decode::decodes_via_mosaic(Path::new(path));
    let settings = oni_settings::sidecar::read(Path::new(path), mosaic, true).map_err(|e| e.to_string())?.settings;
    let mut decoded = render::decode_for(Path::new(path), render::Reading::of(&settings), TILE)?;
    if GROUPS.load(Ordering::Relaxed) != id {
        return Err(String::new());
    }
    decoded.linear = reduced(decoded.linear, TILE);
    Ok(Arc::new(engine.prepare(decoded)?))
}

/// Whether the frame's sidecar reads it as a slide, the key its kept preview is found by.
pub(super) fn read_as_slide(path: &str) -> bool {
    let mosaic = oni_decode::decodes_via_mosaic(Path::new(path));
    oni_settings::sidecar::read(Path::new(path), mosaic, true).is_ok_and(|l| render::Reading::of(&l.settings).slide)
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
        let cell = cell_at(ask.scale);
        let (wx, wy, ww, wh) = ask.window;
        let size = (ww, wh);
        let Ok(mut guard) = engine.lock() else { continue };
        let at = |i: usize, tile| {
            let (x, y) = place(cells[i], tile, cell);
            (x - wx as i32, y - wy as i32)
        };
        let Ok(sheet) = guard.render_laid(&present, cell.0, &at, size) else { continue };
        // The histogram's sheet: every tile at one scale, so a zoom never moves what it counts.
        let counted = ask.counted.as_ref().and_then(|all| {
            let cells: Vec<usize> = all.iter().enumerate().filter_map(|(i, t)| t.as_ref().map(|_| i)).collect();
            let present: Vec<Laid> = all.iter().flatten().map(|(n, s)| Laid::Frame(n.as_ref(), s)).collect();
            let cell = cell_at(COUNTED_SCALE);
            guard.render_laid(&present, cell.0, &|i, tile| place(cells[i], tile, cell), sheet_size(all.len(), cell)).ok()
        });
        // The gaps are clear, so the screen's encoding lays the canvas's ground under them.
        drop(guard);
        let (group, revision, at, pipeline, laid) = (ask.group, ask.revision, ask.at, started.elapsed(), (ask.window, ask.scale));
        with_shell(move |c| c.grid_landed(group, revision, at, sheet, pipeline, laid, counted));
    }
}
