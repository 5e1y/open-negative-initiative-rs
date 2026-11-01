//! Edit Together: the frames selected in the gallery opened as one, five to a row, each its own render
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
#[path = "together/numbers.rs"]
mod numbers;
#[path = "together/print.rs"]
mod print;
#[path = "together/plate.rs"]
mod plate;
#[path = "together/logo.rs"]
mod logo;
#[path = "together/sources.rs"]
mod sources;
use drawing::{draw_grids, tile_source};
use sheet::{Sheet, Window, laid, reduced, scale_for, source_side};
pub(in crate::app) use numbers::ranks;
pub(in crate::app) use print::{Captions, printed};

/// Frames lie on a white sheet when every one of them is a positive.
pub(in crate::app) fn positive<'a>(settings: impl IntoIterator<Item = &'a PipelineSettings>) -> bool {
    let mut any = false;
    settings.into_iter().all(|s| {
        any = true;
        s.mode == oni_settings::ConversionMode::Positive
    }) && any
}

/// Where frames of these sizes lie on their contact sheet, one still unmeasured on 3:2, the logo's
/// band and the sheet's size, in the full sheet's pixels.
pub(in crate::app) fn sheet_places(sizes: &[Option<(f64, f64)>]) -> (Vec<(f64, f64, f64, f64)>, (f64, f64, f64, f64), (f64, f64)) {
    let s = laid(&sizes.iter().map(|s| s.unwrap_or(UNKNOWN)).collect::<Vec<_>>());
    (s.pictures, s.band, s.size)
}

/// The logo's band for a sheet `width` wide, in `ink`, as the canvas shows it.
pub(in crate::app) fn logo_image(width: usize, ink: [f32; 3]) -> slint::Image {
    let plate = logo::drawn(width, ink);
    image(&plate.rgba8(), plate.width, plate.height)
}

/// The full sheet's size in whole pixels for frames of these sizes, as their contact sheet is drawn.
pub(in crate::app) fn sheet_size(sizes: &[(f64, f64)]) -> (f64, f64) {
    let (w, h) = laid(sizes).drawn(1.0);
    (f64::from(w), f64::from(h))
}

/// Tiles a row, each tile's longest side and the gap between them, in pixels.
const COLUMNS: usize = 5;
const TILE: usize = 1024;
const GAP: usize = 30;
/// The sheet's margin and the band under each picture where its number lies, the rows' spacing,
/// the margin under the last row, and the numbers' size.
pub(in crate::app) const MARGIN: usize = 2 * GAP;
const ROWS_APART: usize = 4 * GAP;
const FOOT: usize = 4 * MARGIN;
pub(in crate::app) const NUMBER: usize = 56;
/// The logo's band across the sheet's bottom, under the foot and inside the margin: the logo a tile
/// wide, 246 by 64, stands 266 px tall.
const BAND: usize = 270;
/// A frame's proportions until its thumbnail or its source lands: 35 mm's.
const UNKNOWN: (f64, f64) = (3.0, 2.0);
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
    /// Each frame's place on its roll, and the sheet as its frames' proportions lay it.
    numbers: Vec<Option<usize>>,
    sheet: Sheet,
    /// Whether the sheet was last set white, under frames all positives.
    positive: Option<bool>,
    asked: Option<(u64, Option<oni_settings::id::Id>, usize, usize, u64, Window)>,
    /// The sheet shown: its size, the box of the full sheet it covers and the scale it was drawn at.
    shown: Option<((usize, usize), Window, f64)>,
    /// The revision the last grid shown was drawn at, and each grid's time from its ask to the canvas.
    drawn: u64,
    latencies: Vec<Duration>,
}

static GROUPS: AtomicU64 = AtomicU64::new(0);

impl Group {
    /// The sheet's ground in the display's light: white under frames all positives, else black.
    pub(in crate::app) fn ground(&self) -> f32 {
        if self.positive == Some(true) { 1.0 } else { 0.0 }
    }

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
    /// The sheet laid, the share of it drawn as the view shows it, and the box of it drawn.
    sheet: Sheet,
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
        let (stand_ins, numbers, sheet) = (vec![None; paths.len()], vec![None; paths.len()], laid(&vec![UNKNOWN; paths.len()]));
        let group = Group { id, paths, negatives, stand_ins, numbers, sheet, positive: None, asked: None, shown: None, drawn: 0, latencies: Vec::new() };
        self.state.borrow_mut().together = Some(group);
        if fresh {
            self.state.borrow_mut().viewport.fit();
        }
        self.stand_in(id, &lacking);
        self.decode_together(id, lacking);
        self.number_frames(id);
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
        let Some((drawn, full)) = self.state.borrow().together.as_ref().map(|g| (g.sheet.drawn(scale), g.sheet.size)) else { return };
        let p = self.state.borrow().viewport.placement(full, view).image;
        // Canvas pixels a pixel of the sheet drawn, which shows its box at that box's place.
        let k = p.2 / f64::from(drawn.0);
        let (x, y, w, h) = (f64::from(window.0), f64::from(window.1), f64::from(window.2), f64::from(window.3));
        let whole = (window.2, window.3) == drawn;
        self.lay(Some(Placed { x: p.0 + x * k, y: p.1 + y * k, width: w * k, height: h * k, full, whole, at_full_resolution: true }), size);
        self.publish_numbers(p, full.0);
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
        self.unnumbered();
        self.state.borrow_mut().viewport.fit();
        let mut s = self.state.borrow_mut();
        s.requests.rendered = 0;
        s.requests.requested_view = None;
    }

    pub(in crate::app) fn together_shown(&self) -> bool {
        self.state.borrow().together.is_some()
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
        let mut flipped = None;
        let ask = (|| {
            let mut s = self.state.borrow_mut();
            let g = s.together.as_mut()?;
            // Each frame as framed, its crop and its quarter turns, else its thumbnail's proportions.
            let settings = |path: &String| if Some(path) == open.as_ref() { Some(&lead) } else { live.get(path) };
            let proportions: Vec<(f64, f64)> = g
                .paths
                .iter()
                .zip(&g.negatives)
                .zip(&g.stand_ins)
                .map(|((path, n), stand_in)| match (n, settings(path), stand_in) {
                    (Some(n), Some(s), _) => crate::export::cropped_size((f64::from(n.size().0), f64::from(n.size().1)), s),
                    (_, _, Some(p)) => (f64::from(p.extent().w), f64::from(p.extent().h)),
                    _ => UNKNOWN,
                })
                .collect();
            g.sheet = laid(&proportions);
            let positive = positive(g.paths.iter().filter_map(settings));
            if g.positive != Some(positive) {
                g.positive = Some(positive);
                flipped = Some((g.id, g.sheet.band.2 as usize, positive));
            }
            let full = g.sheet.size;
            let placed = view.map(|v| viewport.placement(full, v));
            let scale = scale_for(placed.map_or(full.0, |p| p.image.2), full.0);
            // The view's part of the sheet, in the drawn sheet's pixels: only the tiles it meets are drawn.
            let drawn = g.sheet.drawn(scale);
            let visible = placed.map_or((0.0, 0.0, f64::from(drawn.0), f64::from(drawn.1)), |p| {
                let k = f64::from(drawn.0) / p.image.2;
                (p.visible.0 * k, p.visible.1 * k, p.visible.2 * k, p.visible.3 * k)
            });
            let (shows, window) = g.sheet.seen(scale, visible);
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
            Some(Ask { group: g.id, revision, at: Instant::now(), tiles, sheet: g.sheet.clone(), scale, window, counted: moved.then_some(all) })
        })();
        if let Some((id, band, positive)) = flipped {
            self.sheet_polarity(id, band, positive);
        }
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
