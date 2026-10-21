//! Edit Together: the frames selected in the gallery opened as one, six to a row, each its own render
//! at a tile's side; the store lays every step on all of them.

use super::*;
use oni_settings::PipelineSettings;
use std::sync::atomic::AtomicUsize;
use std::sync::{Condvar, LazyLock, Mutex};

/// Tiles a row, each tile's longest side and the gap between them, in pixels.
const COLUMNS: usize = 6;
const TILE: usize = 300;
const GAP: usize = 12;
/// Decodes at once while a group opens: a half-size RAW holds about 100 MB until it is reduced.
const DECODERS: usize = 3;

/// The frames shown together, in the gallery's order, each one's source once decoded.
struct Group {
    id: u64,
    paths: Vec<String>,
    negatives: Vec<Option<Arc<Negative>>>,
    asked: Option<(u64, usize)>,
    shown: Option<(usize, usize)>,
    /// The revision the last grid shown was drawn at, and each grid's time from its ask to the canvas.
    drawn: u64,
    latencies: Vec<Duration>,
}

thread_local! {
    static GROUP: RefCell<Option<Group>> = const { RefCell::new(None) };
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
}

/// The latest grid asked, which the drawing thread takes; a newer one replaces it unseen.
static SLOT: LazyLock<(Mutex<Option<Ask>>, Condvar)> = LazyLock::new(|| (Mutex::new(None), Condvar::new()));
static DRAWING: std::sync::Once = std::sync::Once::new();

impl Ctl {
    /// Opens `paths` together around the frame just opened, or leaves any group for two frames fewer.
    pub(in crate::app) fn edit_together(self: &Rc<Self>, paths: Vec<String>) {
        if paths.len() < 2 {
            return self.leave_together();
        }
        let open = self.state.borrow().open.clone();
        let others = paths.iter().filter(|p| Some(*p) != open.as_ref()).cloned().collect();
        if let Some(e) = self.state.borrow_mut().store.editor() {
            e.set_together(others);
        }
        let id = GROUPS.fetch_add(1, Ordering::Relaxed) + 1;
        let count = paths.len();
        GROUP.with(|g| *g.borrow_mut() = Some(Group { id, paths: paths.clone(), negatives: vec![None; count], asked: None, shown: None, drawn: 0, latencies: Vec::new() }));
        self.decode_together(id, paths);
        self.request_render();
    }

    /// Back to one frame: the store edits it alone and the canvas waits for its own render.
    pub(in crate::app) fn leave_together(&self) {
        if GROUP.with(|g| g.borrow_mut().take()).is_none() {
            return;
        }
        GROUPS.fetch_add(1, Ordering::Relaxed);
        if let Some(e) = self.state.borrow_mut().store.editor() {
            e.set_together(Vec::new());
        }
        let mut s = self.state.borrow_mut();
        s.rendered = 0;
        s.requested_view = None;
    }

    pub(in crate::app) fn together_shown(&self) -> bool {
        GROUP.with(|g| g.borrow().is_some())
    }

    /// Each frame's source at a tile's side, a few decoded at once off this thread.
    fn decode_together(&self, id: u64, paths: Vec<String>) {
        let next = Arc::new(AtomicUsize::new(0));
        let paths = Arc::new(paths);
        for _ in 0..DECODERS.min(paths.len()) {
            let (engine, next, paths) = (self.engine.clone(), next.clone(), paths.clone());
            std::thread::spawn(move || {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(path) = paths.get(i) else { return };
                    if GROUPS.load(Ordering::Relaxed) != id {
                        return;
                    }
                    let Ok(negative) = tile_source(&engine, path) else { continue };
                    with_shell(move |c| c.tile_decoded(id, i, negative));
                }
            });
        }
    }

    fn tile_decoded(self: &Rc<Self>, id: u64, i: usize, negative: Arc<Negative>) {
        let landed = GROUP.with(|g| match g.borrow_mut().as_mut() {
            Some(g) if g.id == id => {
                g.negatives[i] = Some(negative);
                true
            }
            _ => false,
        });
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
        let ask = GROUP.with(|g| {
            let mut g = g.borrow_mut();
            let g = g.as_mut()?;
            let key = (revision, g.negatives.iter().flatten().count());
            if g.asked == Some(key) {
                return None;
            }
            g.asked = Some(key);
            let tiles = g
                .paths
                .iter()
                .zip(&g.negatives)
                .map(|(path, n)| {
                    let settings = if Some(path) == open.as_ref() { Some(lead.clone()) } else { live.get(path).cloned() };
                    n.clone().zip(settings)
                })
                .collect();
            Some(Ask { group: g.id, revision, at: Instant::now(), tiles, screen, gpu })
        });
        match ask {
            Some(ask) => self.ask_grid(ask),
            None => {
                if let Some(size) = GROUP.with(|g| g.borrow().as_ref().and_then(|g| g.shown)) {
                    self.lay(None, size);
                }
            }
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
        GROUP.with(|g| g.borrow().as_ref().map_or((0, 0, Vec::new()), |g| (g.paths.len(), g.negatives.iter().flatten().count(), g.latencies.clone())))
    }

    pub(crate) fn together_drawn(&self) -> u64 {
        GROUP.with(|g| g.borrow().as_ref().map_or(0, |g| g.drawn))
    }

    fn grid_landed(&self, group: u64, revision: u64, at: Instant, picture: Picture, pipeline: Duration) {
        let size = match &picture {
            Picture::Pixels { linear, .. } => (linear.width, linear.height),
            Picture::Resident(sheet) => (sheet.extent().w as usize, sheet.extent().h as usize),
        };
        let count = GROUP.with(|g| match g.borrow_mut().as_mut() {
            Some(g) if g.id == group => {
                g.shown = Some(size);
                g.drawn = revision;
                g.latencies.push(at.elapsed());
                Some(g.paths.len())
            }
            _ => None,
        });
        let (Some(count), Some(w)) = (count, self.window()) else { return };
        let c = w.global::<Canvas>();
        self.lay(None, size);
        let on_gpu = self.state.borrow().gpu.is_some();
        match picture {
            Picture::Pixels { rgba8, .. } if !on_gpu => c.set_frame(image(&rgba8, size.0, size.1)),
            picture => {
                let frame = format!("{count} frames together");
                self.encode(picture, Shown { kind: Kind::Notch, asked: at, pipeline, frame }, true);
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

/// `linear` averaged down to `side` on its longest side, each output pixel the mean of its footprint.
fn reduced(linear: render::Linear, side: usize) -> render::Linear {
    let (w, h) = (linear.width, linear.height);
    let long = w.max(h);
    if long <= side {
        return linear;
    }
    let (ow, oh) = ((w * side).div_ceil(long).max(1), (h * side).div_ceil(long).max(1));
    let mut pixels = Vec::with_capacity(ow * oh);
    for y in 0..oh {
        let (y0, y1) = (y * h / oh, ((y + 1) * h / oh).max(y * h / oh + 1));
        for x in 0..ow {
            let (x0, x1) = (x * w / ow, ((x + 1) * w / ow).max(x * w / ow + 1));
            let mut sum = [0.0f32; 4];
            for row in y0..y1 {
                for p in &linear.pixels[row * w + x0..row * w + x1] {
                    (0..4).for_each(|c| sum[c] += p[c]);
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as f32;
            pixels.push(sum.map(|v| v / n));
        }
    }
    render::Linear { width: ow, height: oh, pixels }
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
        let size = sheet_size(ask.tiles.len());
        let Ok(mut guard) = engine.lock() else { continue };
        let Ok(sheet) = guard.render_laid(&present, TILE, &|i, tile| place(cells[i], tile), size) else { continue };
        // The gaps are clear, so the screen's encoding lays the canvas's ground under them.
        let picture = if ask.gpu {
            Picture::Resident(sheet)
        } else {
            let linear = guard.read_sheet(&sheet);
            let rgba8 = render::encoded(&linear, &Params::new(ask.screen, size.0, size.1, seed), 1);
            Picture::Pixels { linear, rgba8 }
        };
        drop(guard);
        let (group, revision, at, pipeline) = (ask.group, ask.revision, ask.at, started.elapsed());
        with_shell(move |c| c.grid_landed(group, revision, at, picture, pipeline));
    }
}

/// A sheet of `count` tiles six to a row, with the gaps between them.
fn sheet_size(count: usize) -> (u32, u32) {
    let columns = count.clamp(1, COLUMNS);
    let rows = count.div_ceil(COLUMNS).max(1);
    ((columns * TILE + (columns - 1) * GAP) as u32, (rows * TILE + (rows - 1) * GAP) as u32)
}

/// The corner of tile `i` of `size`, centred in its cell.
fn place(i: usize, size: (u32, u32)) -> (i32, i32) {
    let (cx, cy) = ((i % COLUMNS) * (TILE + GAP), (i / COLUMNS) * (TILE + GAP));
    let (w, h) = (size.0 as usize, size.1 as usize);
    ((cx + TILE.saturating_sub(w) / 2) as i32, (cy + TILE.saturating_sub(h) / 2) as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reduction_keeps_the_mean_and_the_longest_side() {
        let pixels = (0..600 * 400).map(|i| if i % 2 == 0 { [1.0, 0.0, 0.5, 1.0] } else { [0.0, 1.0, 0.5, 1.0] }).collect();
        let r = reduced(render::Linear { width: 600, height: 400, pixels }, TILE);
        assert_eq!((r.width, r.height), (300, 200));
        assert!(r.pixels.iter().all(|p| (p[0] - 0.5).abs() < 1e-6 && (p[1] - 0.5).abs() < 1e-6), "{:?}", r.pixels[0]);
        // Twin: a picture already small is kept as it is, not averaged.
        let small = reduced(render::Linear { width: 2, height: 1, pixels: vec![[1.0; 4], [0.0; 4]] }, TILE);
        assert_eq!(small.pixels, vec![[1.0; 4], [0.0; 4]]);
    }

    #[test]
    fn seven_tiles_lay_two_rows_of_six_each_centred_in_its_cell() {
        assert_eq!(sheet_size(7), ((6 * TILE + 5 * GAP) as u32, (2 * TILE + GAP) as u32));
        assert_eq!(sheet_size(3), ((3 * TILE + 2 * GAP) as u32, TILE as u32), "a short row is only as wide as its tiles");
        assert_eq!(place(0, (300, 200)), (0, 50), "a landscape tile centred in its cell's height");
        assert_eq!(place(6, (200, 300)), (50, (TILE + GAP) as i32), "the seventh opens the second row");
        // Twin: the second tile's cell starts past the first's and the gap.
        assert_eq!(place(1, (300, 300)), ((TILE + GAP) as i32, 0));
    }
}
