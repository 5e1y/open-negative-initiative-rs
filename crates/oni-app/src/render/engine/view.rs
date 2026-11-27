//! The canvas's render: the region the view shows, at the scale it shows it, kept as one texture.

use super::*;
use oni_i18n::tr;

impl Engine {
    /// `Pipeline.apply` left on the GPU in one texture for the display kernel to read. With a view,
    /// only its visible region, from the full frame once built, else the preview, each reduced once to
    /// the scale shown, so a notch computes no hidden pixel.
    pub fn resident(&mut self, negative: &Negative, settings: &PipelineSettings, view: Option<&View>) -> Result<(Source, Option<Placed>), String> {
        self.fitted.provisional = false;
        // Another photo's scans made ahead go as it opens.
        self.fitted.ahead_of(Image::new(&negative.preview).key());
        let (image, region, placed) = self.composed(negative, settings, view)?;
        // A pan keeps the recipe: the last render's pixels the new region still covers are copied, the rest rendered.
        let key = image.key();
        let before = self.fitted.shown.iter().find(|(k, _)| *k == key).map(|(_, s)| s.clone());
        let source = match (before, region) {
            (Some(before), Some(region)) => self.graph.materialise_over(&image, region, &before).map_err(|e| e.to_string())?,
            _ => self.one_texture(&image, region)?,
        };
        self.fitted.shown.retain(|(k, _)| *k != key);
        if self.fitted.shown.len() == SHOWN {
            self.fitted.shown.remove(0);
        }
        self.fitted.shown.push((key, source.clone()));
        Ok((source, placed))
    }

    /// Compare's before, `resident` on `reference`, kept while its recipe and region hold, so a notch
    /// renders it again only when what it reads moved; none drops the kept one. The reference keeps the
    /// geometry, so the region and its place are the after's, which the canvas lays once for both.
    pub fn before(&mut self, negative: &Negative, reference: Option<&PipelineSettings>, view: Option<&View>) -> Result<Option<Source>, String> {
        let Some(reference) = reference else {
            self.fitted.before = None;
            return Ok(None);
        };
        let (image, region, _) = self.composed(negative, reference, view)?;
        let key = (image.key(), region);
        if let Some((k, source)) = &self.fitted.before
            && *k == key
        {
            return Ok(Some(source.clone()));
        }
        let source = self.one_texture(&image, region)?;
        self.fitted.before = Some((key, source.clone()));
        Ok(Some(source))
    }

    /// The recipe `resident` materialises, the region it keeps and where it lands.
    fn composed(&mut self, negative: &Negative, settings: &PipelineSettings, view: Option<&View>) -> Result<(Image, Option<Rect>, Option<Placed>), String> {
        let request = Request { full_width: Some(negative.full_width), ..Request::default() };
        self.read_custom(settings);
        let digital = settings.mode.is_digital();
        let Some(view) = view else {
            let preview = match digital {
                true => self.scanned(negative, settings, None)?.0,
                false => negative.preview.clone(),
            };
            let image = match digital {
                true => self.pipeline.on_scan(&mut self.graph, &preview, settings, &request),
                false => {
                    let inputs = inputs_of(&self.patches, &self.custom, settings, negative.seed, None);
                    self.pipeline.image(&mut self.graph, &negative.preview, settings, &request, &inputs)
                }
            };
            let image = image.map_err(|e| e.to_string())?;
            return Ok((self.proof.laid(&mut self.graph, image, settings)?, None, None));
        };
        let ratio = negative.size().0 as f64 / negative.full_width;
        // Stage 0's cut on the full frame's own grid: the preview's, scaled up, is a few pixels off 1:1.
        let whole = Rect::new(0, 0, negative.full_width.round() as u32, negative.full_height.round() as u32);
        let cut = oni_pipeline::geometry::extent(&settings.geometry, whole, 1.0);
        let full = (f64::from(cut.w), f64::from(cut.h));
        let vp = view.viewport;
        let scale = vp.effective_scale(full, view.size);
        let placement = vp.placement(full, view.size);
        let lighter = lighter(view.coarse, view.size);
        // A digital photo is read off its scan, made once while its film's settings hold, as a scanned negative is;
        // its full scan only once the view asks past the preview's resolution, as at 100 %.
        let (preview, scan_full) = match digital {
            true => self.scanned(negative, settings, view.full.as_ref().filter(|_| scale * lighter > ratio))?,
            false => (negative.preview.clone(), view.full.clone()),
        };
        let (base, base_ratio) = match &scan_full {
            Some(f) => (f.clone(), 1.0),
            None => (preview.clone(), ratio),
        };
        let e = base.extent();
        let long = e.w.max(e.h) as usize;
        // While a hand moves the view the region is rendered at most GESTURE_PIXELS and shown stretched,
        // so a 5K canvas costs a gesture what a small window does; the sharp render follows the stop.
        let side = ((long as f64 * (scale * lighter / base_ratio).min(1.0)).round() as usize).max(1);
        // Reduced on the way in, so the chain pulls only the region's share of the level it reads; a
        // reduction the canvas nearly holds whole is kept, so a notch at fit reads it ready.
        let (source, reduce) = match side < long {
            true => {
                let (level, reduce) = self.level(&base, side, long)?;
                let e = level.extent();
                let whole = f64::from(e.w) * f64::from(e.h) * reduce * reduce;
                if kept_whole(whole, view.size, lighter) { (self.reduced(&level, reduce)?, 1.0) } else { (level, reduce) }
            }
            false => (base, 1.0),
        };
        let request = Request { scale: reduce, ..request };
        let image = match digital {
            true => self.pipeline.on_scan(&mut self.graph, &source, settings, &request),
            false => {
                let inputs = inputs_of(&self.patches, &self.custom, settings, negative.seed, None);
                self.pipeline.image(&mut self.graph, &source, settings, &request, &inputs)
            }
        };
        let image = image.map_err(|e| e.to_string())?;
        let image = self.proof.laid(&mut self.graph, image, settings)?;
        let extent = image.extent();
        let stretch = scale * full.0 / extent.w as f64;
        let (vx, vy, vw, vh) = placement.visible;
        // One size a view, the visible part's with a margin each side, so a pan keeps every texture it renders
        // into and the picture it moves at once shows no bare edge before the next render lands.
        let span = |v: f64, len: f64, origin: i32, end: i32| {
            let margin = (len / stretch * PAN_MARGIN).ceil() as i32;
            let size = ((len / stretch).ceil() as i32 + 1 + 2 * margin).min(end - origin);
            let start = (origin + (v / stretch).floor() as i32 - margin).clamp(origin, end - size);
            (start, start + size)
        };
        let ((x0, x1), (y0, y1)) = (span(vx, vw, extent.x, extent.right()), span(vy, vh, extent.y, extent.bottom()));
        let region = Rect::from_corners(x0, y0, x1, y1);
        if region.is_empty() {
            return Err(tr("the image is off the canvas").into());
        }
        let placed = Placed {
            x: placement.image.0 + f64::from(region.x - extent.x) * stretch,
            y: placement.image.1 + f64::from(region.y - extent.y) * stretch,
            width: f64::from(region.w) * stretch,
            height: f64::from(region.h) * stretch,
            full,
            whole: region == extent,
            at_full_resolution: scan_full.is_some(),
        };
        Ok((image, Some(region), Some(placed)))
    }

    /// A digital photo's scan from its preview, and from its full frame when one is given; each made once while the
    /// film's settings hold, in half floats as a decode is. While the scan is held back the preview's stands in for a
    /// full one not yet made, marked provisional, so a gesture or a film lit in its menu renders at the preview's cost.
    fn scanned(&mut self, negative: &Negative, settings: &PipelineSettings, full: Option<&Source>) -> Result<(Source, Option<Source>), String> {
        let request = Request { full_width: Some(negative.full_width), ..Request::default() };
        let inputs = inputs_of(&self.patches, &self.custom, settings, negative.seed, Some(&negative.preview));
        let image = self.pipeline.scan(&mut self.graph, &negative.preview, settings, &request, &inputs).map_err(|e| e.to_string())?;
        let preview = match &self.fitted.scan {
            Some((key, kept)) if *key == image.key() => kept.clone(),
            _ => {
                let made = match self.fitted.ahead.iter().position(|(k, _)| *k == image.key()) {
                    Some(i) => self.fitted.ahead.remove(i).1,
                    None => self.graph.materialise_half_cached(&image, Some(&mut self.fitted.scan_stages)).map_err(|e| e.to_string())?,
                };
                // The scan left stays ahead, so the hand going back finds it made.
                if let Some(left) = self.fitted.scan.replace((image.key(), made.clone())) {
                    self.fitted.keep_ahead(left);
                }
                // A film's settings moved: the full scan kept is no longer this film's.
                self.fitted.full_scan = None;
                made
            }
        };
        let Some(full) = full else { return Ok((preview, None)) };
        let image = self.pipeline.scan(&mut self.graph, full, settings, &request, &inputs).map_err(|e| e.to_string())?;
        if let Some((key, kept)) = &self.fitted.full_scan
            && *key == image.key()
        {
            return Ok((preview, Some(kept.clone())));
        }
        if self.fitted.held {
            self.fitted.provisional = true;
            return Ok((preview, None));
        }
        // The last full scan goes before the next is made: two would hold twice a full frame.
        self.fitted.full_scan = None;
        let made = self.graph.materialise_half(&image).map_err(|e| e.to_string())?;
        self.fitted.full_scan = Some((image.key(), made.clone()));
        Ok((preview, Some(made)))
    }

    /// A digital photo's preview scan on `settings` made ahead of the hand, as a render would make it; false where
    /// it is already made or the photo is no digital one.
    pub fn preload(&mut self, negative: &Negative, settings: &PipelineSettings) -> Result<bool, String> {
        if settings.film().is_none() {
            return Ok(false);
        }
        let request = Request { full_width: Some(negative.full_width), ..Request::default() };
        let inputs = inputs_of(&self.patches, &self.custom, settings, negative.seed, Some(&negative.preview));
        let image = self.pipeline.scan(&mut self.graph, &negative.preview, settings, &request, &inputs).map_err(|e| e.to_string())?;
        let key = image.key();
        self.fitted.ahead_of(Image::new(&negative.preview).key());
        if self.fitted.scan.as_ref().is_some_and(|(k, _)| *k == key) || self.fitted.ahead.iter().any(|(k, _)| *k == key) {
            return Ok(false);
        }
        let made = self.graph.materialise_half_cached(&image, Some(&mut self.fitted.scan_stages)).map_err(|e| e.to_string())?;
        self.fitted.keep_ahead((key, made));
        Ok(true)
    }

    /// Whether the next render holds a digital photo's full scan back for a rest; a render's own when none runs it.
    pub fn hold_scan(&mut self, held: bool) {
        self.fitted.held = held;
    }

    /// Whether the last render read the preview's scan in place of a full one it waits for.
    pub fn provisional(&self) -> bool {
        self.fitted.provisional
    }

    pub(super) fn one_texture(&mut self, image: &Image, region: Option<Rect>) -> Result<Source, String> {
        let region = region.unwrap_or_else(|| image.extent());
        let source = self.graph.materialise_cached(image, region, Some(&mut self.cache)).map_err(|e| e.to_string())?;
        if source.view().is_none() {
            return Err(tr("the render is wider than one texture").into());
        }
        Ok(source)
    }

    /// The smallest halving of `base` still at or past `side` on its long side, and the factor left to
    /// reach `side` from it, under two: a zoom's every step reads a level near the screen's size.
    pub(super) fn level(&mut self, base: &Source, side: usize, long: usize) -> Result<(Source, f64), String> {
        let id = base.id();
        let at = match self.fitted.halves.iter().position(|(k, _)| *k == id) {
            Some(at) => at,
            None => {
                if self.fitted.halves.len() >= BASES {
                    self.fitted.halves.remove(0);
                }
                self.fitted.halves.push((id, Vec::new()));
                self.fitted.halves.len() - 1
            }
        };
        let (mut from, mut from_long) = (base.clone(), long);
        for level in 0.. {
            if from_long / 2 < side.max(MIN_HALF) {
                break;
            }
            if self.fitted.halves[at].1.len() == level {
                let half = self.graph.materialise_half(&Image::new(&from).lanczos(0.5)).map_err(|e| e.to_string())?;
                self.fitted.halves[at].1.push(half);
            }
            from = self.fitted.halves[at].1[level].clone();
            let e = from.extent();
            from_long = e.w.max(e.h) as usize;
        }
        Ok((from, side as f64 / from_long as f64))
    }

    /// The whole picture `COUNTED_SIDE` long from the preview, for the histogram, which so reads the same
    /// pixels at fit and zoomed; the kept one while its recipe holds, so a picture counted is known by its id.
    pub fn counted(&mut self, negative: &Negative, settings: &PipelineSettings) -> Result<Source, String> {
        let (w, h) = negative.size();
        let scale = (COUNTED_SIDE / w.max(h) as f64).min(1.0);
        let request = Request { scale, full_width: Some(negative.full_width), ..Request::default() };
        self.read_custom(settings);
        // A digital photo's read off the scan the canvas keeps.
        let image = match settings.mode.is_digital() {
            true => {
                let (preview, _) = self.scanned(negative, settings, None)?;
                self.pipeline.on_scan(&mut self.graph, &preview, settings, &request)
            }
            false => {
                let inputs = inputs_of(&self.patches, &self.custom, settings, negative.seed, None);
                self.pipeline.image(&mut self.graph, &negative.preview, settings, &request, &inputs)
            }
        };
        let image = image.map_err(|e| e.to_string())?;
        let image = self.proof.laid(&mut self.graph, image, settings)?;
        let key = image.key();
        if let Some((k, source)) = &self.fitted.counted
            && *k == key
        {
            return Ok(source.clone());
        }
        let source = self.graph.materialise(&image).map_err(|e| e.to_string())?;
        self.fitted.counted = Some((key, source.clone()));
        Ok(source)
    }

    /// `level` reduced by `reduce` whole, the last one kept.
    fn reduced(&mut self, level: &Source, reduce: f64) -> Result<Source, String> {
        let key = (level.id(), reduce.to_bits());
        if let Some((k, source)) = &self.fitted.kept
            && *k == key
        {
            return Ok(source.clone());
        }
        let source = self.graph.materialise(&Image::new(level).lanczos(reduce)).map_err(|e| e.to_string())?;
        self.fitted.kept = Some((key, source.clone()));
        Ok(source)
    }
}

/// The scale a view renders at, relative to the screen's: under a hand, at most `GESTURE_PIXELS`.
fn lighter(coarse: bool, size: (f64, f64)) -> f64 {
    let margins = (1.0 + 2.0 * PAN_MARGIN) * (1.0 + 2.0 * PAN_MARGIN);
    if coarse { (GESTURE_PIXELS / (size.0 * size.1 * margins)).sqrt().min(1.0) } else { 1.0 }
}

/// A reduction holding `whole` pixels is made whole and kept when it holds at most `KEPT_WHOLE` canvases
/// rendered `lighter`: one exact scale is kept, so a gesture reducing more each step would never reuse it.
fn kept_whole(whole: f64, size: (f64, f64), lighter: f64) -> bool {
    whole <= KEPT_WHOLE * size.0 * size.1 * lighter * lighter
}

/// The share of the view rendered past each of its sides, which a pan reveals before its render lands.
const PAN_MARGIN: f64 = 0.125;
/// The pixels a view rendered during a gesture holds at most, about what a 1440p window's view does.
const GESTURE_PIXELS: f64 = 4.0e6;
/// The long side of the picture the histogram counts.
const COUNTED_SIDE: f64 = 1024.0;
/// A reduction kept whole when it holds at most this many canvases of pixels.
const KEPT_WHOLE: f64 = 2.0;

/// The smallest halving kept: under it a reduction costs little from the level above.
const MIN_HALF: usize = 64;
/// The bases whose halvings are kept: the preview and the full frame, so neither evicts the other.
const BASES: usize = 2;
/// The canvas's renders kept for a pan: the lighter one a gesture shows and the sharp one it ends on.
const SHOWN: usize = 2;

/// The halvings of the bases the canvas reads, and the last whole reduction made from one.
#[derive(Default)]
pub(crate) struct Fitting {
    halves: Vec<(u64, Vec<Source>)>,
    kept: Option<((u64, u64), Source)>,
    /// The histogram's picture and Compare's before, each with the recipe it was made from.
    counted: Option<(u64, Source)>,
    before: Option<((u64, Option<Rect>), Source)>,
    /// The canvas's last renders, each with its recipe.
    shown: Vec<(u64, Source)>,
    /// A digital photo's scans, from the preview and from the full frame, each with its recipe; whether a hand holds
    /// the setting rendered and whether the last render stood the preview's scan in for a full one.
    scan: Option<(u64, Source)>,
    full_scan: Option<(u64, Source)>,
    /// The preview scan's stages a film notch leaves as they are, a band each, so the notch reruns only what follows.
    scan_stages: Vec<oni_graph::Cache>,
    /// Preview scans made ahead for the notches beside the hand's, the photo's whose preview key is `ahead_for`.
    ahead: Vec<(u64, Source)>,
    ahead_for: Option<u64>,
    held: bool,
    provisional: bool,
}

/// The preview scans kept ahead: four of a 47 Mpx photo hold about 85 MB.
pub(crate) const AHEAD: usize = 4;

impl Fitting {
    /// The scans ahead dropped when they are another photo's.
    fn ahead_of(&mut self, photo: u64) {
        if self.ahead_for != Some(photo) {
            self.ahead.clear();
            self.ahead_for = Some(photo);
        }
    }

    /// A scan kept ahead, the oldest let go past `AHEAD`.
    fn keep_ahead(&mut self, scan: (u64, Source)) {
        self.ahead.retain(|(k, _)| *k != scan.0);
        if self.ahead.len() == AHEAD {
            self.ahead.remove(0);
        }
        self.ahead.push(scan);
    }
}

#[cfg(test)]
mod tests;
