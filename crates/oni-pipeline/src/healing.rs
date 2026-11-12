//! A frame's corrections laid together: their patches gathered on one sheet, their strokes on another,
//! each step of their making one pass for them all, however many the frame holds.

use crate::corrections;
use crate::stages::{Blur, Patched, Stroke};
use oni_graph::stages::{AnchoredLanczos, Axis};
use oni_graph::{Batch, Graph, Image, Laid, Rect, Source};
use oni_render::{Gaussian, StrokeMask};
use oni_settings::axis::pipeline::BLUR_FLOOR;
use oni_settings::frame::Correction;
use std::collections::HashMap;

type Result<T> = std::result::Result<T, oni_graph::Error>;

/// The width the patches, and the strokes' points, are packed across.
const WIDTH: u32 = 4096;
/// The recipes kept: the canvas, the histogram's picture, the two input readings, the loupe and a
/// search each name a frame's corrections at their own scale.
const KEPT: usize = 8;

/// The patches the batches read, copied on sheets: each one's sheet and place there, by its own source.
struct Sheets {
    sources: Vec<Source>,
    at: HashMap<u64, (usize, Rect)>,
}

/// A frame's corrections at one scale, ready to lay on its scan.
struct Recipe {
    framed: Rect,
    scale: u64,
    /// Each correction laid, with the source its patch was read from.
    members: Vec<(Correction, u64)>,
    /// The first of the sheets it reads, which names them all.
    sheets: u64,
    /// A batch for each run of corrections whose patches share a sheet, in the corrections' order.
    laid: Vec<Laid>,
    used: u64,
}

/// The sheets and the recipes last made, kept while renders name them again: a notch builds none.
#[derive(Default)]
pub(crate) struct Healing {
    sheets: Option<Sheets>,
    recipes: Vec<Recipe>,
    renders: u64,
}

/// Rectangles laid in rows `WIDTH` across, on a sheet no side of which passes `limit`.
#[derive(Default)]
struct Rows {
    x: u32,
    y: u32,
    high: u32,
    wide: u32,
}

impl Rows {
    /// Where a rectangle of `size` goes; none once the sheet is full.
    fn place(&mut self, (w, h): (u32, u32), limit: u32) -> Option<(i32, i32)> {
        let (x, y, high) = if self.x > 0 && self.x + w > WIDTH.min(limit) { (0, self.y + self.high, 0) } else { (self.x, self.y, self.high) };
        if (x, y) != (0, 0) && y + high.max(h) > limit {
            return None;
        }
        (self.x, self.y, self.high, self.wide) = (x + w, y, high.max(h), self.wide.max(x + w));
        Some((x as i32, y as i32))
    }

    fn size(&self) -> (u32, u32) {
        (self.wide.max(1), (self.y + self.high).max(1))
    }
}

/// Every patch of `named` copied on as few sheets as the texture side allows, in their order.
fn gathered(graph: &mut Graph, named: &[(&Correction, &Source)]) -> Result<Sheets> {
    let mut sheets = Sheets { sources: Vec::new(), at: HashMap::new() };
    let mut pending = named.iter().map(|(_, patch)| *patch).peekable();
    while pending.peek().is_some() {
        let (mut rows, mut parts) = (Rows::default(), Vec::new());
        while let Some(patch) = pending.peek() {
            let Some(at) = rows.place((patch.extent().w, patch.extent().h), graph.max_side()) else { break };
            sheets.at.insert(patch.id(), (sheets.sources.len(), Rect::new(at.0, at.1, patch.extent().w, patch.extent().h)));
            parts.push((*patch, at));
            pending.next();
        }
        sheets.sources.push(graph.gathered(&parts, rows.size())?);
    }
    Ok(sheets)
}

impl Healing {
    /// `image`, the scan at `scale`, with each of `named` laid: its patch through its feathered
    /// stroke, in the corrections' order.
    pub(crate) fn laid(&mut self, graph: &mut Graph, image: Image, named: &[(&Correction, &Source)], scale: f64) -> Result<Image> {
        if named.is_empty() {
            return Ok(image);
        }
        self.renders += 1;
        let (framed, now) = (image.extent(), self.renders);
        let held = |r: &Recipe| r.framed == framed && r.scale == scale.to_bits() && r.members.len() == named.len() && r.members.iter().zip(named).all(|((c, id), (n, patch))| *id == patch.id() && c == *n);
        if let Some(r) = self.recipes.iter_mut().find(|r| held(r)) {
            r.used = now;
            return Ok(r.laid.iter().fold(image, |image, laid| image.then_laid(laid)));
        }
        if !self.sheets.as_ref().is_some_and(|s| named.iter().all(|(_, patch)| s.at.contains_key(&patch.id()))) {
            self.sheets = Some(gathered(graph, named)?);
        }
        let sheets = self.sheets.as_ref().expect("gathered above");
        let on = |(_, patch): &(&Correction, &Source)| sheets.at[&patch.id()];
        let mut laid = Vec::new();
        for run in named.chunk_by(|a, b| on(a).0 == on(b).0) {
            let at = run.iter().map(|member| on(member).1).collect();
            laid.push(recipe(graph, &sheets.sources[on(&run[0]).0], at, run, framed, scale)?);
        }
        // A recipe on sheets since replaced can serve no render again.
        let current = sheets.sources[0].id();
        self.recipes.retain(|r| r.sheets == current);
        if self.recipes.len() >= KEPT {
            let oldest = (0..self.recipes.len()).min_by_key(|&i| self.recipes[i].used).expect("a recipe");
            self.recipes.swap_remove(oldest);
        }
        let members = named.iter().map(|(c, patch)| ((*c).clone(), patch.id())).collect();
        let image = laid.iter().fold(image, |image, laid| image.then_laid(laid));
        self.recipes.push(Recipe { framed, scale: scale.to_bits(), members, sheets: current, laid, used: now });
        Ok(image)
    }
}

/// `named`, whose patches lie at `at` on `sheet`, as one batch laid: each patch reduced to `scale` and
/// moved to its place, each stroke's hard mask feathered, each patch blended through it over its tile.
fn recipe(graph: &mut Graph, sheet: &Source, at: Vec<Rect>, named: &[(&Correction, &Source)], framed: Rect, scale: f64) -> Result<Laid> {
    let sizes: Vec<(u32, u32)> = at.iter().map(|r| (r.w, r.h)).collect();
    let placed: Vec<(Rect, (f64, f64))> = named.iter().zip(&sizes).map(|((c, _), size)| corrections::placed(c, *size, framed, scale)).collect();
    let mut patches = Batch::of(sheet, at);
    if scale != 1.0 {
        let along = |axis: Axis| -> Vec<Option<AnchoredLanczos>> {
            let one = |(size, (_, shift)): (&(u32, u32), &(Rect, (f64, f64)))| {
                let (from, shift) = if axis == Axis::X { (size.0, shift.0) } else { (size.1, shift.1) };
                Some(AnchoredLanczos { axis, origin: 0, from, scale, shift })
            };
            sizes.iter().zip(&placed).map(one).collect()
        };
        patches = patches.then(along(Axis::X)).then(along(Axis::Y));
    }
    let patches = patches.moved(placed.iter().map(|(tile, _)| [tile.x, tile.y]).collect());
    let tiles = patches.extents().to_vec();

    // Every stroke's points in rows of one source, a stroke a run of one row.
    let points: Vec<Vec<[f32; 4]>> = named.iter().map(|(c, _)| corrections::stroke_points(c, framed)).collect();
    let mut rows = Rows::default();
    let places: Option<Vec<(i32, i32)>> = points.iter().map(|p| rows.place((p.len() as u32, 1), graph.max_side())).collect();
    let (places, size) = (places.ok_or(oni_graph::Error::TooLarge)?, rows.size());
    if size.0 > graph.max_side() {
        return Err(oni_graph::Error::TooLarge);
    }
    let mut row = vec![[0.0f32; 4]; (size.0 * size.1) as usize];
    for (p, (x, y)) in points.iter().zip(&places) {
        let first = (*y as u32 * size.0) as usize + *x as usize;
        row[first..first + p.len()].copy_from_slice(p);
    }
    let strokes = graph.upload(size.0, size.1, &row)?;
    let hard = |((c, _), (p, tile)): (&(&Correction, &Source), (&Vec<[f32; 4]>, &Rect))| Some(Stroke { k: StrokeMask { radius: (c.radius * scale) as f32, count: p.len() }, extent: *tile });
    let masks = Batch::of(&strokes, points.iter().zip(&places).map(|(p, (x, y))| Rect::new(*x, *y, p.len() as u32, 1)).collect()).then(named.iter().zip(points.iter().zip(&tiles)).map(hard).collect());
    // `CIGaussianBlur` returns its input bit for bit at or under the floor: such a mask stays hard.
    let feathered = |vertical: bool| -> Vec<Option<Blur>> {
        let one = |sigma: f64| (sigma > BLUR_FLOOR).then(|| Blur(Gaussian { sigma: sigma as f32, vertical, transparent: true }));
        named.iter().map(|(c, _)| one(corrections::feather(c, scale))).collect()
    };
    let masks = masks.then(feathered(false)).then(feathered(true));
    Ok(Laid::new(tiles.iter().map(|&tile| Patched { patch: tile, mask: tile }).collect(), &[patches, masks]))
}
