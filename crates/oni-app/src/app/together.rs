//! Edit Together: the frames selected in the gallery opened as one, six to a row, each its own render
//! at a tile's side; the store lays every step on all of them.

use super::*;
use crate::render::Laid;
use oni_settings::PipelineSettings;
use std::sync::atomic::AtomicUsize;
use crate::latest::Latest;

#[path = "together/sheet.rs"]
mod sheet;
#[path = "together/drawing.rs"]
mod drawing;
use drawing::{draw_grids, tile_source};
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
    /// Each frame's thumbnail on the GPU, shown in its tile until its source is decoded.
    stand_ins: Vec<Option<oni_graph::Source>>,
    asked: Option<(u64, Option<oni_settings::id::Id>, usize, usize, u64, Window)>,
    /// The sheet shown: its size, the box of the full sheet it covers and the scale it was drawn at.
    shown: Option<((usize, usize), Window, f64)>,
    /// The revision the last grid shown was drawn at, and each grid's time from its ask to the canvas.
    drawn: u64,
    latencies: Vec<Duration>,
}

static GROUPS: AtomicU64 = AtomicU64::new(0);

impl Group {
    /// The frames whose sources are decoded, and how many there are.
    pub(in crate::app) fn loaded(&self) -> (usize, usize) {
        (self.negatives.iter().flatten().count(), self.paths.len())
    }

    /// The frames whose sources are decoded, in the gallery's order.
    pub(in crate::app) fn decoded(&self) -> impl Iterator<Item = &String> {
        self.paths.iter().zip(&self.negatives).filter(|(_, n)| n.is_some()).map(|(p, _)| p)
    }
}

/// What a tile shows: its frame on its settings, or its thumbnail while the frame decodes.
#[derive(Clone)]
enum Tile {
    Frame(Arc<Negative>, PipelineSettings),
    StandIn(oni_graph::Source),
}

/// A grid to draw: its group, each tile in order, the screen it is encoded for.
struct Ask {
    group: u64,
    revision: u64,
    at: Instant,
    tiles: Vec<Option<Tile>>,
    /// The share of the full sheet drawn, as the view shows it, and the box of it drawn.
    scale: f64,
    window: Window,
    /// Every tile, when the settings or the sources moved: the histogram's sheet, drawn whole at a set scale.
    counted: Option<Vec<Option<(Arc<Negative>, PipelineSettings)>>>,
}

/// The latest grid asked, which the drawing thread takes; a newer one replaces it unseen.
static SLOT: Latest<Ask> = Latest::new();
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
        let stand_ins = vec![None; paths.len()];
        self.state.borrow_mut().together = Some(Group { id, paths, negatives, stand_ins, asked: None, shown: None, drawn: 0, latencies: Vec::new() });
        if fresh {
            self.state.borrow_mut().viewport.fit();
        }
        self.stand_in(id, &lacking);
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
        s.requests.rendered = 0;
        s.requests.requested_view = None;
    }

    pub(in crate::app) fn together_shown(&self) -> bool {
        self.state.borrow().together.is_some()
    }

    /// Each frame's source at a tile's side, a few at once off this thread: a frame the editor holds
    /// is reduced from its preview at once, the others decoded, those held first.
    fn decode_together(&self, id: u64, mut paths: Vec<(usize, String)>) {
        let previews = self.state.borrow().previews.clone();
        let held: Vec<bool> = paths.iter().map(|(_, p)| previews.lock().expect("previews").holds(p)).collect();
        let mut order: Vec<usize> = (0..paths.len()).collect();
        order.sort_by_key(|&k| !held[k]);
        paths = order.into_iter().map(|k| paths[k].clone()).collect();
        let next = Arc::new(AtomicUsize::new(0));
        let paths = Arc::new(paths);
        for _ in 0..DECODERS.min(paths.len()) {
            let (engine, next, paths, previews) = (self.engine.clone(), next.clone(), paths.clone(), previews.clone());
            std::thread::spawn(move || {
                // A group left while a decode waits for its place gives that place to the next group's.
                crate::decode_gate::only_while(move || GROUPS.load(Ordering::Relaxed) == id);
                loop {
                    let Some((i, path)) = paths.get(next.fetch_add(1, Ordering::Relaxed)).cloned() else { return };
                    if GROUPS.load(Ordering::Relaxed) != id {
                        return;
                    }
                    let slide = drawing::read_as_slide(&path);
                    let preview = previews.lock().expect("previews").get(&path, slide);
                    let made = match preview {
                        Some(p) => engine.lock().and_then(|mut e| e.tile_of(&p, TILE)).map(Arc::new),
                        None => tile_source(&engine, &path, id),
                    };
                    let Ok(negative) = made else { continue };
                    with_shell(move |c| c.tile_decoded(id, i, negative));
                }
            });
        }
    }

    /// Each lacking frame's thumbnail, read from disk and laid on the GPU off this thread: a tile shows
    /// it at once rather than nothing until its frame decodes.
    fn stand_in(&self, id: u64, lacking: &[(usize, String)]) {
        let (support, held): (PathBuf, Vec<(usize, String)>) = {
            let s = self.state.borrow();
            (s.support.clone(), lacking.iter().filter_map(|(i, p)| s.fingerprint(p).map(|f| (*i, f.to_string()))).collect())
        };
        let engine = self.engine.clone();
        std::thread::spawn(move || {
            for (i, fingerprint) in held {
                if GROUPS.load(Ordering::Relaxed) != id {
                    return;
                }
                let Some((rgba, w, h)) = crate::thumbnails::load(&support, &fingerprint, Screen::Srgb) else { continue };
                let mut pixels: Vec<[f32; 4]> = rgba.chunks_exact(4).map(|p| [p[0], p[1], p[2], p[3]].map(|v| f32::from(v) / 255.0)).collect();
                oni_colour::source::Source::of(None, None, false).linearise(&mut pixels, false);
                let Ok(picture) = engine.lock().and_then(|mut e| e.picture(w as u32, h as u32, &pixels)) else { continue };
                with_shell(move |c| c.tile_stood_in(id, i, picture));
            }
        });
    }

    fn tile_stood_in(self: &Rc<Self>, id: u64, i: usize, picture: oni_graph::Source) {
        let landed = match self.state.borrow_mut().together.as_mut() {
            Some(g) if g.id == id && g.negatives[i].is_none() => {
                g.stand_ins[i] = Some(picture);
                true
            }
            _ => false,
        };
        if landed {
            self.request_render();
        }
    }

    fn tile_decoded(self: &Rc<Self>, id: u64, i: usize, negative: Arc<Negative>) {
        let landed = match self.state.borrow_mut().together.as_mut() {
            Some(g) if g.id == id => {
                g.negatives[i] = Some(negative);
                g.stand_ins[i] = None;
                true
            }
            _ => false,
        };
        if landed {
            self.request_render();
            self.balance_ahead();
        }
    }

    /// The grid asked again once a step or a source moved it; else only laid again where the view is.
    pub(in crate::app) fn render_together(self: &Rc<Self>) {
        let (revision, lead, open) = {
            let s = self.state.borrow();
            (s.store.revision(), s.store.settings().clone(), s.open.clone())
        };
        let live: HashMap<String, PipelineSettings> =
            self.state.borrow().store.editor_ref().map(|e| e.together_live().into_iter().collect()).unwrap_or_default();
        // A hovered preset previews on every frame, as a click applies it to the whole selection.
        let (lead, live) = {
            let s = self.state.borrow();
            (s.previewed(lead), live.into_iter().map(|(path, settings)| (path, s.previewed(settings))).collect::<HashMap<_, _>>())
        };
        let (view, viewport) = (self.canvas_size(), self.state.borrow().viewport);
        let hovered = self.state.borrow().shelf.hovered;
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
            let key = (revision, hovered, g.negatives.iter().flatten().count(), g.stand_ins.iter().flatten().count(), (scale * 8.0) as u64, window);
            if g.asked == Some(key) {
                return None;
            }
            let moved = g.asked.is_none_or(|a| (a.0, a.1, a.2) != (key.0, key.1, key.2));
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
            let tiles = all
                .iter()
                .zip(&g.stand_ins)
                .zip(&shows)
                .map(|((frame, stand_in), shown)| match frame {
                    Some((n, s)) => Some(Tile::Frame(n.clone(), s.clone())),
                    None => stand_in.clone().map(Tile::StandIn),
                }.filter(|_| *shown))
                .collect();
            Some(Ask { group: g.id, revision, at: Instant::now(), tiles, scale, window, counted: moved.then_some(all) })
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
        SLOT.put(ask);
    }

    /// The frames together, their sources decoded and each grid's latency, for the bench.
    pub(crate) fn together_stats(&self) -> (usize, usize, Vec<Duration>) {
        self.state.borrow().together.as_ref().map_or((0, 0, Vec::new()), |g| (g.paths.len(), g.negatives.iter().flatten().count(), g.latencies.clone()))
    }

    pub(crate) fn together_drawn(&self) -> u64 {
        self.state.borrow().together.as_ref().map_or(0, |g| g.drawn)
    }

    fn grid_landed(&self, group: u64, revision: u64, at: Instant, sheet: oni_graph::Source, pipeline: Duration, (window, scale): (Window, f64), counted: Option<oni_graph::Source>) {
        let size = (sheet.extent().w as usize, sheet.extent().h as usize);
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
        let frame = format!("{count} frames together");
        self.encode(sheet, Shown { kind: Kind::Notch, asked: at, pipeline, frame }, counted);
        c.set_busy(false);
        c.set_status(tr!("{count} frames together", count = count).into());
    }
}
