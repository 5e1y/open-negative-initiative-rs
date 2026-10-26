//! The picture of a demo: the owner's photograph through the app's own engine, as the canvas
//! and the export render it, encoded for an sRGB screen undithered so a flat sky stays one colour.

use std::path::{Path, PathBuf};

use oni_app::display::{Params, Screen};
use oni_app::render::{self, Device, Engine, Linear, Negative, Reading};
use oni_graph::{Rect, Source};
use oni_pipeline::Request;
use oni_settings::PipelineSettings;

mod heal;
pub use heal::magnified;

/// RGBA rows, 8 bits a channel, opaque.
#[derive(Clone, PartialEq)]
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// The photograph opened read-only: its preview for whole-frame pictures, its full decode
/// for a detail at one pixel a pixel. Nothing is written beside it.
pub struct Photo {
    engine: Engine,
    path: PathBuf,
    preview: Option<Negative>,
    full: Option<Source>,
}

/// The owner's photograph every demo is made of, read-only: a Lumix RW2 scan of a colour negative.
pub const DEFAULT: &str = "/Volumes/T7/Scans/2026 Fin Mars - Leica M6 rouleau 1/5200K/P1071951.RW2";
/// Its sidecar's settings, copied here so a demo never follows the photo's later edits.
const BASE: &[u8] = include_bytes!("../scenes/P1071951.json");

/// The demos' photograph: `ONI_DEMO_PHOTO`, else `DEFAULT`.
pub fn source() -> PathBuf {
    std::env::var_os("ONI_DEMO_PHOTO").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(DEFAULT))
}

/// The photograph's own settings, every demo's start.
pub fn base() -> PipelineSettings {
    settings(BASE).expect("the demos' settings decode")
}

/// The demos' framing of the whole photograph: its left three quarters, the sails, the cap and the
/// sand, without the dark sail's black.
pub fn framed(mut s: PipelineSettings) -> PipelineSettings {
    s.geometry.crop = oni_settings::cg::Rect::new(0.0, 0.0, 0.76, 1.0);
    s
}

/// A sidecar's settings read from bytes held in this crate.
pub fn settings(json: &[u8]) -> Result<PipelineSettings, String> {
    let source = oni_settings::codec::Source { mosaic: &|| true, plain: &|| false };
    oni_settings::codec::decode_with(json, &source, true).map(|d| d.settings).map_err(|e| format!("{e:?}"))
}

impl Photo {
    pub fn open(path: &Path) -> Result<Photo, String> {
        if !path.is_file() {
            return Err(format!("{} is missing: mount it or point ONI_DEMO_PHOTO at it", path.display()));
        }
        let device = Device::open()?;
        Ok(Photo { engine: Engine::on(&device), path: path.to_path_buf(), preview: None, full: None })
    }

    /// Every picture after this proofed on `space`, the paper's white shown, as the canvas shows a print.
    pub fn proof(&mut self, space: oni_colour::Space) {
        self.engine.proof.wanted = Some(oni_app::proof::wanted(space, false));
    }

    /// The whole frame at `s`, scaled to cover `width` × `height` and cut about its centre.
    pub fn cover(&mut self, s: &PipelineSettings, width: u32, height: u32) -> Result<Rgba, String> {
        if self.preview.is_none() {
            let decoded = render::decode_for(&self.path, Reading::of(s), render::PREVIEW_SIDE)?;
            self.preview = Some(self.engine.prepare(decoded)?);
        }
        let negative = self.preview.as_ref().expect("prepared");
        // A first render tells the frame's shape; the second lands its short side on the box.
        let probe = self.engine.render(negative, s, Some(64))?;
        let aspect = probe.width as f64 / probe.height as f64;
        let (w, h) = (width as f64, height as f64);
        let mut side = (if aspect >= 1.0 { w.max(h * aspect) } else { h.max(w / aspect) }).ceil() as usize + 1;
        // The engine rounds the short side: a render a pixel short is made again a little larger.
        loop {
            let linear = self.engine.render(negative, s, Some(side))?;
            if (linear.width >= width as usize && linear.height >= height as usize) || side > 4 * (width + height) as usize {
                return Ok(cut(&linear, width, height));
            }
            side += 2;
        }
    }

    /// The preview decoded again and changed by `alter` before it is laid, for a demo that stages
    /// how the scan itself was made; `cover` renders from it after.
    pub fn restage(&mut self, s: &PipelineSettings, alter: impl FnOnce(&mut Linear)) -> Result<(), String> {
        let mut decoded = render::decode_for(&self.path, Reading::of(s), render::PREVIEW_SIDE)?;
        alter(&mut decoded.linear);
        self.preview = Some(self.engine.prepare(decoded)?);
        Ok(())
    }

    /// `width` × `height` of the full-resolution render, one pixel a pixel, centred on `at`, a
    /// fraction of the rendered frame.
    pub fn detail(&mut self, s: &PipelineSettings, at: (f64, f64), width: u32, height: u32) -> Result<Rgba, String> {
        if self.full.is_none() {
            let decoded = render::decode_full(&self.path, Reading::of(s))?;
            self.full = Some(self.engine.full(decoded)?);
        }
        let source = self.full.as_ref().expect("decoded");
        let image = self.engine.unproofed(source, s, &Request::default())?;
        let e = image.extent();
        let x = e.x + ((e.w as f64 * at.0) as i32 - width as i32 / 2).clamp(0, e.w as i32 - width as i32);
        let y = e.y + ((e.h as f64 * at.1) as i32 - height as i32 / 2).clamp(0, e.h as i32 - height as i32);
        let (graph, _) = self.engine.parts();
        let pixels = graph.render(&image, Rect::new(x, y, width, height), None).map_err(|e| e.to_string())?;
        graph.purge();
        Ok(encoded(&Linear { width: width as usize, height: height as usize, pixels }))
    }
}

/// `width` × `height` about the centre of `l`.
fn cut(l: &Linear, width: u32, height: u32) -> Rgba {
    let (w, h) = (width as usize, height as usize);
    let x0 = l.width.saturating_sub(w) / 2;
    let y0 = l.height.saturating_sub(h) / 2;
    let mut pixels = Vec::with_capacity(w * h);
    for y in 0..h.min(l.height) {
        let row = (y0 + y) * l.width + x0;
        pixels.extend_from_slice(&l.pixels[row..row + w.min(l.width)]);
    }
    encoded(&Linear { width: w.min(l.width), height: h.min(l.height), pixels })
}

fn encoded(l: &Linear) -> Rgba {
    let mut params = Params::new(Screen::Srgb, l.width as u32, l.height as u32, 0);
    params.dither = 0.0;
    Rgba { width: l.width as u32, height: l.height as u32, pixels: render::encoded(l, &params, 1) }
}
