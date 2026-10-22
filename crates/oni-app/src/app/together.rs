//! Edit Together: the frames selected in the gallery opened as one, six to a row, each its own render
//! at a tile's side; the store lays every step on all of them.

use super::*;
use oni_settings::PipelineSettings;
use std::sync::atomic::AtomicUsize;
use std::sync::{Condvar, LazyLock, Mutex};

#[path = "together/sheet.rs"]
mod sheet;
use sheet::{Window, cell_at, place, reduced, scale_for, seen, sheet_size};

/// Tiles a row, each tile's longest side and the gap between them, in pixels.
const COLUMNS: usize = 6;
const TILE: usize = 1024;
const GAP: usize = 30;
/// The share of the full sheet the histogram counts, an eighth: 128 px a tile.
const COUNTED_SCALE: f64 = 0.125;
/// Decodes at once while a group opens: a half-size RAW holds about 100 MB until it is reduced.
const DECODERS: usize = 3;

/// The frames shown together, in the gallery's order, each one's source once decoded.
pub(in crate::app) struct Group {
    id: u64,
    paths: Vec<String>,
    negatives: Vec<Option<Arc<Negative>>>,
    asked: Option<(u64, usize, u64, Window)>,
    /// The sheet shown: its size, the box of the full sheet it covers and the scale it was drawn at.
    shown: Option<((usize, usize), Window, f64)>,
    /// The revision the last grid shown was drawn at, and each grid's time from its ask to the canvas.
    drawn: u64,
    latencies: Vec<Duration>,
}

static GROUPS: AtomicU64 = AtomicU64::new(0);

/// A grid to draw: its group, each tile's source and settings in order, the screen it is encoded for.
struct Ask {
    group: u64,
    revision: u64,
    at: Instant,
    tiles: Vec<Option<(Arc<Negative>, PipelineSettings)>>,
    screen: Screen,
    /// The canvas encodes on the GPU, which takes the grid as light; the software one takes bytes.
    gpu: bool,
    /// The share of the full sheet drawn, as the view shows it, and the box of it drawn.
    scale: f64,
    window: Window,
    /// Every tile, when the settings or the sources moved: the histogram's sheet, drawn whole at a set scale.
    counted: Option<Vec<Option<(Arc<Negative>, PipelineSettings)>>>,
}

/// The latest grid asked, which the drawing thread takes; a newer one replaces it unseen.
static SLOT: LazyLock<(Mutex<Option<Ask>>, Condvar)> = LazyLock::new(|| (Mutex::new(None), Condvar::new()));
static DRAWING: std::sync::Once = std::sync::Once::new();

impl Ctl {
    /// Opens `paths` together around the frame just opened, or leaves any group for two frames fewer;
    /// the selection stays theirs, and the sources the group shown decoded are kept.
    pub(in crate::app) fn edit_together(self: &Rc<Self>, paths: Vec<String>) {
        if paths.len() < 2 {
            return self.leave_together();
        }
        let open = self.state.borrow().open.clone();
        let others = paths.iter().filter(|p| Some(*p) != open.as_ref()).cloned().collect();
        {
            let mut s = self.state.borrow_mut();
            if let Some(e) = s.store.editor() {
                e.set_together(others);
            }
            s.picks.selection = paths.iter().cloned().collect();
        }
        let id = GROUPS.fetch_add(1, Ordering::Relaxed) + 1;
        let (negatives, fresh) = {
            let s = self.state.borrow();
            let g = s.together.as_ref();
            let kept = |p: &String| g.and_then(|g| g.paths.iter().position(|q| q == p).and_then(|i| g.negatives[i].clone()));
            (paths.iter().map(kept).collect::<Vec<_>>(), g.is_none())
        };
        let lacking: Vec<(usize, String)> = paths.iter().enumerate().filter(|(i, _)| negatives[*i].is_none()).map(|(i, p)| (i, p.clone())).collect();
        self.state.borrow_mut().together = Some(Group { id, paths, negatives, asked: None, shown: None, drawn: 0, latencies: Vec::new() });
        if fresh {
            self.state.borrow_mut().viewport.fit();
        }
        self.decode_together(id, lacking);
        self.request_render();
        self.publish_tiles();
    }

    /// The selection made beside the open photo: two frames or more show together at once, the open one
    /// leading when it is among them; one frame shows alone again.
    pub(in crate::app) fn follow_selection(self: &Rc<Self>) {
        let (selected, open) = {
            let s = self.state.borrow();
            (s.selected(), s.open.clone())
        };
        if selected.len() < 2 {
            if self.together_shown() {
                self.leave_together();
                self.request_render();
            }
            return;
        }
        let lead = open.filter(|o| selected.contains(o)).unwrap_or_else(|| selected[0].clone());
        if self.state.borrow().open.as_deref() != Some(lead.as_str()) {
            self.open_photo(&lead);
        }
        self.edit_together(selected);
    }

    /// The frames shown together, for the keys that act on all of them.
    pub(in crate::app) fn together_paths(&self) -> Option<Vec<String>> {
        self.state.borrow().together.as_ref().map(|g| g.paths.clone())
    }

    /// The sheet where the view lays it, zoomed and panned as a contact sheet: the view reasons on the
    /// full sheet whatever share of it was drawn.
    fn lay_sheet(&self, (size, window, scale): ((usize, usize), Window, f64)) {
        let Some(view) = self.canvas_size() else { return };
        let Some(count) = self.state.borrow().together.as_ref().map(|g| g.paths.len()) else { return };
        let (drawn, full) = (sheet_size(count, cell_at(scale)), sheet_size(count, cell_at(1.0)));
        let full = (f64::from(full.0), f64::from(full.1));
        let p = self.state.borrow().viewport.placement(full, view).image;
        // Canvas pixels a pixel of the sheet drawn, which shows its box at that box's place.
        let k = p.2 / f64::from(drawn.0);
        let (x, y, w, h) = (f64::from(window.0), f64::from(window.1), f64::from(window.2), f64::from(window.3));
        let whole = (window.2, window.3) == drawn;
        self.lay(Some(Placed { x: p.0 + x * k, y: p.1 + y * k, width: w * k, height: h * k, full, whole, at_full_resolution: true }), size);
    }

    /// Back to one frame: the store edits it alone and the canvas waits for its own render.
    pub(in crate::app) fn leave_together(&self) {
        if self.state.borrow_mut().together.take().is_none() {
            return;
        }
        GROUPS.fetch_add(1, Ordering::Relaxed);
        if let Some(e) = self.state.borrow_mut().store.editor() {
            e.set_together(Vec::new());
        }
        self.state.borrow_mut().viewport.fit();
        let mut s = self.state.borrow_mut();
        s.rendered = 0;
        s.requested_view = None;
    }

    pub(in crate::app) fn together_shown(&self) -> bool {
        self.state.borrow().together.is_some()
    }

    /// Each frame's source at a tile's side, a few at once off this thread: a frame the editor holds
    /// is reduced from its preview at once, the others decoded, those held first.
    fn decode_together(&self, id: u64, mut paths: Vec<(usize, String)>) {
        let previews = self.state.borrow().previews.clone();
        let held: Vec<bool> = paths.iter().map(|(_, p)| previews.lock().expect("previews").get(p).is_some()).collect();
        let mut order: Vec<usize> = (0..paths.len()).collect();
        order.sort_by_key(|&k| !held[k]);
        paths = order.into_iter().map(|k| paths[k].clone()).collect();
        let next = Arc::new(AtomicUsize::new(0));
        let paths = Arc::new(paths);
        for _ in 0..DECODERS.min(paths.len()) {
            let (engine, next, paths, previews) = (self.engine.clone(), next.clone(), paths.clone(), previews.clone());
            std::thread::spawn(move || {
                loop {
                    let Some((i, path)) = paths.get(next.fetch_add(1, Ordering::Relaxed)).cloned() else { return };
                    if GROUPS.load(Ordering::Relaxed) != id {
                        return;
                    }
                    let preview = previews.lock().expect("previews").get(&path);
                    let made = match preview {
                        Some(p) => engine.lock().and_then(|mut e| e.tile_of(&p, TILE)).map(Arc::new),
                        None => tile_source(&engine, &path),
                    };
                    let Ok(negative) = made else { continue };
                    with_shell(move |c| c.tile_decoded(id, i, negative));
                }
            });
        }
    }

    fn tile_decoded(self: &Rc<Self>, id: u64, i: usize, negative: Arc<Negative>) {
        let landed = match self.state.borrow_mut().together.as_mut() {
            Some(g) if g.id == id => {
                g.negatives[i] = Some(negative);
                true
            }
            _ => false,
        };
        if landed {
            self.request_render();
        }
    }

    /// The grid asked again once a step or a source moved it; else only laid again where the view is.
    pub(in crate::app) fn render_together(self: &Rc<Self>) {
        let (revision, screen, gpu, lead, open) = {
            let s = self.state.borrow();
            (s.store.revision(), s.screen, s.gpu.is_some(), s.store.settings().clone(), s.open.clone())
        };
        let live: HashMap<String, PipelineSettings> =
            self.state.borrow().store.editor_ref().map(|e| e.together_live().into_iter().collect()).unwrap_or_default();
        let (view, viewport) = (self.canvas_size(), self.state.borrow().viewport);
        let ask = (|| {
            let mut s = self.state.borrow_mut();
            let g = s.together.as_mut()?;
            let full = sheet_size(g.paths.len(), cell_at(1.0));
            let full = (f64::from(full.0), f64::from(full.1));
            let placed = view.map(|v| viewport.placement(full, v));
            let scale = scale_for(placed.map_or(full.0, |p| p.image.2), full.0);
            // The view's part of the sheet, in the drawn sheet's pixels: only the tiles it meets are drawn.
            let drawn = sheet_size(g.paths.len(), cell_at(scale));
            let visible = placed.map_or((0.0, 0.0, f64::from(drawn.0), f64::from(drawn.1)), |p| {
                let k = f64::from(drawn.0) / p.image.2;
                (p.visible.0 * k, p.visible.1 * k, p.visible.2 * k, p.visible.3 * k)
            });
            let (shows, window) = seen(g.paths.len(), cell_at(scale), visible);
            let key = (revision, g.negatives.iter().flatten().count(), (scale * 8.0) as u64, window);
            if g.asked == Some(key) {
                return None;
            }
            let moved = g.asked.is_none_or(|a| (a.0, a.1) != (key.0, key.1));
            g.asked = Some(key);
            let all: Vec<_> = g
                .paths
                .iter()
                .zip(&g.negatives)
                .map(|(path, n)| {
                    let settings = if Some(path) == open.as_ref() { Some(lead.clone()) } else { live.get(path).cloned() };
                    n.clone().zip(settings)
                })
                .collect();
            let tiles = all.iter().zip(&shows).map(|(t, shown)| t.clone().filter(|_| *shown)).collect();
            Some(Ask { group: g.id, revision, at: Instant::now(), tiles, screen, gpu, scale, window, counted: moved.then_some(all) })
        })();
        // The sheet shown follows the view at once, stretched until the one drawn at its new size lands.
        let shown = self.state.borrow().together.as_ref().and_then(|g| g.shown);
        if let Some(size) = shown {
            self.lay_sheet(size);
        }
        if let Some(ask) = ask {
            self.ask_grid(ask);
        }
    }

    fn ask_grid(&self, ask: Ask) {
        let engine = self.engine.clone();
        DRAWING.call_once(move || {
            std::thread::Builder::new().name("together".into()).spawn(move || draw_grids(engine)).expect("together thread");
        });
        *SLOT.0.lock().expect("together slot") = Some(ask);
        SLOT.1.notify_one();
    }

    /// The frames together, their sources decoded and each grid's latency, for the bench.
    pub(crate) fn together_stats(&self) -> (usize, usize, Vec<Duration>) {
        self.state.borrow().together.as_ref().map_or((0, 0, Vec::new()), |g| (g.paths.len(), g.negatives.iter().flatten().count(), g.latencies.clone()))
    }

    pub(crate) fn together_drawn(&self) -> u64 {
        self.state.borrow().together.as_ref().map_or(0, |g| g.drawn)
    }

    fn grid_landed(&self, group: u64, revision: u64, at: Instant, picture: Picture, pipeline: Duration, (window, scale): (Window, f64), counted: Option<oni_graph::Source>) {
        let size = match &picture {
            Picture::Pixels { linear, .. } => (linear.width, linear.height),
            Picture::Resident(sheet) => (sheet.extent().w as usize, sheet.extent().h as usize),
        };
        let count = match self.state.borrow_mut().together.as_mut() {
            Some(g) if g.id == group => {
                g.shown = Some((size, window, scale));
                g.drawn = revision;
                g.latencies.push(at.elapsed());
                Some(g.paths.len())
            }
            _ => None,
        };
        let (Some(count), Some(w)) = (count, self.window()) else { return };
        let c = w.global::<Canvas>();
        self.lay_sheet((size, window, scale));
        let on_gpu = self.state.borrow().gpu.is_some();
        match picture {
            Picture::Pixels { rgba8, .. } if !on_gpu => c.set_frame(image(&rgba8, size.0, size.1)),
            picture => {
                let frame = format!("{count} frames together");
                self.encode(picture, Shown { kind: Kind::Notch, asked: at, pipeline, frame }, counted);
            }
        }
        c.set_busy(false);
        c.set_status(format!("{count} frames together").into());
    }
}

/// A frame decoded at half size when it can, reduced to a tile and laid on the GPU.
fn tile_source(engine: &Shared, path: &str) -> Result<Arc<Negative>, String> {
    let mosaic = oni_decode::decodes_via_mosaic(Path::new(path));
    let settings = oni_settings::sidecar::read(Path::new(path), mosaic, true).map_err(|e| e.to_string())?.settings;
    let mut decoded = render::decode_for(Path::new(path), render::Reading::of(&settings), TILE)?;
    decoded.linear = reduced(decoded.linear, TILE);
    Ok(Arc::new(engine.prepare(decoded)?))
}

/// The drawing thread: the latest grid asked, each tile rendered on its own settings and laid in rows.
fn draw_grids(engine: Shared) {
    let mut seed = 0u32;
    loop {
        let ask = {
            let mut slot = SLOT.0.lock().expect("together slot");
            loop {
                if let Some(ask) = slot.take() {
                    break ask;
                }
                slot = SLOT.1.wait(slot).expect("together slot");
            }
        };
        seed = seed.wrapping_add(1);
        let started = Instant::now();
        let cells: Vec<usize> = ask.tiles.iter().enumerate().filter_map(|(i, t)| t.as_ref().map(|_| i)).collect();
        let present: Vec<(&Negative, &PipelineSettings)> = ask.tiles.iter().flatten().map(|(n, s)| (n.as_ref(), s)).collect();
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
        let counted = ask.counted.as_ref().filter(|_| ask.gpu).and_then(|all| {
            let cells: Vec<usize> = all.iter().enumerate().filter_map(|(i, t)| t.as_ref().map(|_| i)).collect();
            let present: Vec<(&Negative, &PipelineSettings)> = all.iter().flatten().map(|(n, s)| (n.as_ref(), s)).collect();
            let cell = cell_at(COUNTED_SCALE);
            guard.render_laid(&present, cell.0, &|i, tile| place(cells[i], tile, cell), sheet_size(all.len(), cell)).ok()
        });
        // The gaps are clear, so the screen's encoding lays the canvas's ground under them.
        let picture = if ask.gpu {
            Picture::Resident(sheet)
        } else {
            let linear = guard.read_sheet(&sheet);
            let rgba8 = render::encoded(&linear, &Params::new(ask.screen, size.0, size.1, seed), 1);
            Picture::Pixels { linear, rgba8 }
        };
        drop(guard);
        let (group, revision, at, pipeline, laid) = (ask.group, ask.revision, ask.at, started.elapsed(), (ask.window, ask.scale));
        with_shell(move |c| c.grid_landed(group, revision, at, picture, pipeline, laid, counted));
    }
}
