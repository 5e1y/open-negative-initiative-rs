//! The photograph opened as De-dust opens it, the app's reading on it, and its detail shown: a negative
//! through the engine, a finished export as its own values.

use super::*;

/// The app's auto De-dust mask side (`MASK_SIDE` in its reading): the veil's texels, shown pixelated.
pub(super) const MASK_SIDE: usize = 3072;

/// The photograph opened as De-dust opens it: its sidecar's settings without a correction, its full
/// decode on the GPU, and its preview for the tiles' reminders. Nothing is written beside it.
pub struct Scan {
    engine: Shared,
    full: Source,
    preview: Negative,
    pub settings: PipelineSettings,
    /// The framed frame at full resolution, in pixels, and where its extent starts.
    pub size: (u32, u32),
    origin: (i32, i32),
    placing: Placing,
    caches: PathBuf,
    /// A finished photograph's own code values, shown as they are: a JPEG export is never converted.
    as_is: Option<Rgba>,
}

impl Scan {
    pub fn open(path: &Path) -> Result<Scan, String> {
        let plain = oni_decode::route(path) == oni_decode::Branch::Plain;
        let loaded = oni_settings::sidecar::read(path, oni_decode::decodes_via_mosaic(path)).map_err(|e| format!("its sidecar: {e:?}"))?;
        // An export with no edit is read as the positive it is, which the reading takes at its blacks.
        let mut settings = if plain && loaded.is_new { PipelineSettings::rest(oni_settings::ConversionMode::Positive, false) } else { loaded.settings };
        settings.corrections.clear();
        let device = Device::open()?;
        let engine = Engine::shared(&device);
        let caches = std::env::temp_dir().join(format!("oni-welcome-dedust-{}", std::process::id()));
        std::fs::create_dir_all(&caches).map_err(|e| e.to_string())?;
        engine.lock()?.caches = Some(caches.clone());
        let full = engine.full(render::decode_full(path, Reading::of(&settings))?)?;
        let preview = engine.prepare(render::decode_for(path, Reading::of(&settings), render::PREVIEW_SIDE)?)?;
        let e = engine.lock()?.unproofed(&full, &settings, &Request::default())?.extent();
        let scan = full.extent();
        let placing = settings.geometry.placing((f64::from(scan.w), f64::from(scan.h)));
        let as_is = if plain { Some(code_values(path)?) } else { None };
        if as_is.as_ref().is_some_and(|p| (p.width, p.height) != (e.w, e.h)) {
            return Err("the export's pixels and its render differ in size".into());
        }
        Ok(Scan { engine, full, preview, settings, size: (e.w, e.h), origin: (e.x, e.y), placing, caches, as_is })
    }

    /// The app's reading of the framed frame, and how long it took.
    pub fn read(&self) -> Result<(dust::Reading, Duration, Structure), String> {
        let started = Instant::now();
        let (width, pixels) = self.engine.lock()?.framed_whole(&self.full, &self.settings)?;
        let reading = dust::read(&pixels, width, &Limits::of(&self.settings), MASK_SIDE);
        if (reading.width, reading.height) != (self.size.0 as usize, self.size.1 as usize) {
            return Err(format!("the reading's frame is {}×{}, the render's {:?}", reading.width, reading.height, self.size));
        }
        let took = started.elapsed();
        Ok((reading, took, Structure::of(&pixels, width)))
    }

    /// The view of `s` at the strip's pixels, as the canvas zoomed past one to one shows it.
    pub fn detail(&self, s: &PipelineSettings, view: View) -> Result<Rgba, String> {
        let (w, h) = view.region();
        if let Some(whole) = &self.as_is {
            if s.corrections.is_empty() {
                return Ok(zoomed(&crop(whole, view.x, view.y, w, h), view));
            }
            return Ok(zoomed(&self.healed_as_is(s, view)?, view));
        }
        let mut e = self.engine.lock()?;
        let image = e.unproofed(&self.full, s, &Request::default())?;
        let (graph, _) = e.parts();
        let rect = Rect::new(self.origin.0 + view.x as i32, self.origin.1 + view.y as i32, w, h);
        let pixels = graph.render(&image, rect, None).map_err(|e| e.to_string())?;
        graph.purge();
        let picture = photo::encoded(&render::Linear { width: w as usize, height: h as usize, pixels });
        Ok(zoomed(&picture, view))
    }

    /// A finished photograph's view with its patches laid, still never converted: the decoded scan the
    /// patches lie on, framed as the reading reads it, brought back to the screen's encoding.
    fn healed_as_is(&self, s: &PipelineSettings, view: View) -> Result<Rgba, String> {
        let (w, h) = view.region();
        let (width, pixels) = self.engine.lock()?.framed_whole(&self.full, s)?;
        let rows = (view.y as usize..(view.y + h) as usize).flat_map(|y| pixels[y * width + view.x as usize..][..w as usize].iter().copied());
        let mut params = Params::new(Screen::Srgb, w, h, 0);
        params.dither = 0.0;
        // Primaries held, the scan is in the file's own, an sRGB export's: only its curve is laid back.
        if !s.primaries_converted {
            params.rows = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]];
        }
        let linear = render::Linear { width: w as usize, height: h as usize, pixels: rows.collect() };
        Ok(Rgba { width: w, height: h, pixels: render::encoded(&linear, &params, 1) })
    }

    /// How far the healed path strays from the export's own values with nothing laid: 0 to 255.
    pub fn as_is_drift(&self, view: View) -> Result<u8, String> {
        let Some(whole) = &self.as_is else { return Ok(0) };
        let (w, h) = view.region();
        let (a, b) = (crop(whole, view.x, view.y, w, h), self.healed_as_is(&self.settings, view)?);
        Ok(a.pixels.iter().zip(&b.pixels).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0))
    }

    /// Each correction's reminder of its dust, as the tab's tiles show them, for an sRGB screen.
    pub fn originals(&self, s: &PipelineSettings, found: &[Found]) -> Result<Vec<Rgba>, String> {
        let strokes: Vec<(Vec<(f64, f64)>, f64)> = found.iter().map(|f| (f.stroke.clone(), f.radius)).collect();
        let linear = self.engine.lock()?.originals(&self.preview, s, &strokes)?;
        Ok(linear
            .iter()
            .map(|l| {
                let mut params = Params::new(Screen::Srgb, l.width as u32, l.height as u32, 0);
                params.dither = 0.0;
                Rgba { width: l.width as u32, height: l.height as u32, pixels: render::encoded(l, &params, 1) }
            })
            .collect())
    }

    /// The corrections `found` stands for, kept on the scan, as REMOVE writes them.
    pub fn corrections(&self, found: &[Found]) -> Vec<Correction> {
        found.iter().map(|f| Correction { id: Id::new(), stroke: f.stroke.iter().map(|&(x, y)| self.placing.on_scan(Point { x, y })).collect(), radius: f.radius }).collect()
    }

    /// The app's heal on `s`, each patch made in its order; `landed` hears each correction's index once
    /// its patch is written, and the heal's own time to it, `landed`'s own work left out.
    pub fn heal(&self, s: &PipelineSettings, mut landed: impl FnMut(usize, Duration) -> Result<(), String>) -> Result<(), String> {
        let mut skip = std::collections::HashSet::new();
        let (started, mut aside) = (Instant::now(), Duration::ZERO);
        let failed = std::cell::RefCell::new(None::<String>);
        oni_app::heal::ensure(&self.engine, &self.full, &self.caches, &mut skip, || failed.borrow().is_none().then(|| s.clone()), |made| {
            let at = started.elapsed() - aside;
            let result = made.map_err(|(_, e)| e).and_then(|(key, _, _)| {
                let k = s.corrections.iter().position(|c| c.id == key.id).ok_or("a patch for no correction")?;
                let own = Instant::now();
                landed(k, at)?;
                aside += own.elapsed();
                Ok(())
            });
            if let Err(e) = result {
                *failed.borrow_mut() = Some(e);
            }
        });
        failed.into_inner().map_or(Ok(()), Err)
    }
}

impl Drop for Scan {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.caches);
    }
}

/// A speck the reading found, on the framed frame: its box in pixels and the stroke REMOVE writes for it,
/// in fractions of the frame, with its radius in pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub bounds: [u32; 4],
    pub stroke: Vec<(f64, f64)>,
    pub radius: f64,
}

impl Found {
    pub fn of(s: &Speck, size: (u32, u32)) -> Found {
        let (stroke, radius) = dust::stroke(s, size.0 as usize, size.1 as usize);
        Found { bounds: s.bounds, stroke, radius }
    }

    /// The correction's mark box, `bounding` in the app: the stroke's extent padded by its radius, in pixels.
    pub(super) fn mark(&self, size: (u32, u32)) -> [f64; 4] {
        let (w, h) = (f64::from(size.0), f64::from(size.1));
        let (x0, x1) = self.stroke.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.0 * w), b.max(p.0 * w)));
        let (y0, y1) = self.stroke.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.1 * h), b.max(p.1 * h)));
        [x0 - self.radius, y0 - self.radius, x1 - x0 + 2.0 * self.radius, y1 - y0 + 2.0 * self.radius]
    }
}

/// The dust the reading shows at `sensitivity`, as the canvas rings it.
pub fn found(reading: &dust::Reading, sensitivity: f32) -> Vec<Found> {
    let size = (reading.width as u32, reading.height as u32);
    reading.at(sensitivity).into_iter().map(|s| Found::of(s, size)).collect()
}

/// `picture` reduced `by` times to `width` × `height` from its top left, each pixel the mean of the
/// area it stands for, partial pixels by their share, as a canvas zoomed under one to one shows it.
pub fn reduced(picture: &Rgba, by: f32, width: u32, height: u32) -> Rgba {
    // Each output pixel's sources along one axis, with the share of each.
    let spans = |n: u32, limit: u32| -> Vec<Vec<(u32, f32)>> {
        (0..n)
            .map(|o| {
                let (a, b) = (o as f32 * by, (o + 1) as f32 * by);
                (a.floor() as u32..(b.ceil() as u32).min(limit)).map(|i| (i, (b.min(i as f32 + 1.0) - a.max(i as f32)).max(0.0))).collect()
            })
            .collect()
    };
    let (xs, ys) = (spans(width, picture.width), spans(height, picture.height));
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for row in &ys {
        for column in &xs {
            let (mut sum, mut weight) = ([0f32; 4], 0f32);
            for &(py, wy) in row {
                for &(px, wx) in column {
                    let at = ((py * picture.width + px) * 4) as usize;
                    for (c, total) in sum.iter_mut().enumerate() {
                        *total += f32::from(picture.pixels[at + c]) * wx * wy;
                    }
                    weight += wx * wy;
                }
            }
            pixels.extend(sum.map(|v| if weight > 0.0 { (v / weight).round() as u8 } else { 0 }));
        }
    }
    Rgba { width, height, pixels }
}

/// `picture`, a view's region, at the view's zoom.
pub(super) fn zoomed(picture: &Rgba, view: View) -> Rgba {
    if view.zoom >= 1.0 { magnified(picture, view.zoom as u32, view.width, view.height) } else { reduced(picture, 1.0 / view.zoom, view.width, view.height) }
}

/// `w` × `h` of `p` from `(x, y)`.
pub(super) fn crop(p: &Rgba, x: u32, y: u32, w: u32, h: u32) -> Rgba {
    let mut pixels = Vec::with_capacity((w * h * 4) as usize);
    for row in y..y + h {
        let at = ((row * p.width + x) * 4) as usize;
        pixels.extend_from_slice(&p.pixels[at..at + (w * 4) as usize]);
    }
    Rgba { width: w, height: h, pixels }
}

/// An export's own code values, read as the app reads a plain image: an sRGB file's are the screen's.
pub(super) fn code_values(path: &Path) -> Result<Rgba, String> {
    let read = oni_decode::plain::read(path, None).ok_or("the export does not decode")?;
    let srgb = |icc: &Vec<u8>| icc.windows(4).any(|t| t == b"sRGB") || icc.windows(8).any(|t| t == b"\0s\0R\0G\0B");
    if read.icc.as_ref().is_some_and(|icc| !srgb(icc)) {
        return Err("the export's profile is not sRGB: its values are not the screen's".into());
    }
    let pixels = read.pixels.iter().flat_map(|p| [p[0], p[1], p[2]].map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8).into_iter().chain([255])).collect();
    Ok(Rgba { width: read.width as u32, height: read.height as u32, pixels })
}
