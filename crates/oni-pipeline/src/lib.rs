//! `Pipeline.apply`: a frame's settings to its working-space image, every stage in the Swift app's
//! order, as one render-graph recipe that stays on the GPU until a region of it is read.

mod chain;
pub mod conversion;
pub mod corrections;
pub mod film;
pub mod finishing;
pub mod flat_field;
pub mod geometry;
pub mod grain;
pub mod halation;
mod healing;
pub mod look_lab;
pub mod negative;
pub mod oni_look;
pub mod paper;
pub mod frontier_look;
pub mod resident;
pub mod scanner;
pub mod stages;

use oni_graph::{Cache, Graph, Image, Rect, Source};
use oni_render::ColorCube;
use oni_settings::axis::pipeline::BLUR_FLOOR;
use oni_settings::curves::{CurveSet, SaturationCurve};
use oni_settings::frame::FlatFieldMask;
use oni_settings::id::Id;
use oni_settings::tone::Print;
use oni_settings::{LegacyMachine, LegacyPrint, PipelineSettings};
use stages::{Before, DetailColumns, DetailRows, OnGround};
use std::collections::HashMap;

/// What a render is asked for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Request {
    /// What `render` reads back, in the result's pixels; `None` is its whole extent.
    pub region: Option<Rect>,
    /// The source reduced by Lanczos before the chain: a preview's factor, 1 at full resolution.
    pub scale: f64,
    /// The width of the full-resolution frame the source was read from; `None` when it is the source.
    pub full_width: Option<f64>,
    /// A measurement: the output clip and a sheet's ground are left off.
    pub measuring: bool,
}

impl Default for Request {
    fn default() -> Self {
        Request { region: None, scale: 1.0, full_width: None, measuring: false }
    }
}

/// A custom flat-field mask, resident on the graph, for the frame naming its `id`.
pub struct CustomFlatField {
    pub id: String,
    pub mask: Source,
    pub peaks: [f32; 3],
    pub per_channel: bool,
}

/// What the settings name but do not hold: the dust patches, generated once at full resolution and
/// keyed by their correction, a custom flat-field mask, and the photo's grain seed.
#[derive(Default)]
pub struct Inputs<'a> {
    pub patches: HashMap<Id, &'a Source>,
    pub custom_flat_field: Option<&'a CustomFlatField>,
    pub seed: [u32; 2],
}

/// The constant images the chain reads, uploaded once per graph, and the tables last baked.
#[derive(Default)]
pub struct Pipeline {
    valoi: Option<(Source, [f32; 3])>,
    cubes: HashMap<&'static str, (Source, usize)>,
    luts: Vec<(CurveSet, Source, Source)>,
    /// Each virtual film's table, uploaded once per graph.
    films: HashMap<oni_settings::VirtualStock, Source>,
    /// The frames' corrections as last laid.
    healing: healing::Healing,
}

/// Tables kept per pipeline, as two curve sets alternate.
const KEPT_LUTS: usize = 4;

type Result<T> = std::result::Result<T, oni_graph::Error>;

/// A cube's nodes as an image `n²` wide and `n` high: node (r, g, b) at (r + n·g, b).
fn upload_cube(graph: &mut Graph, cube: &ColorCube) -> Result<Source> {
    let n = cube.cube_dimension as u32;
    graph.upload(n * n, n, &cube.cube_data)
}

impl Pipeline {
    pub fn new() -> Self {
        Self::default()
    }

    /// The corrections' sheets and recipes let go, with the frame they were laid on.
    pub fn trim(&mut self) {
        self.healing = Default::default();
    }

    /// The recipe evaluated on the request's region and read back, with that region.
    pub fn render(
        &mut self,
        graph: &mut Graph,
        source: &Source,
        settings: &PipelineSettings,
        request: &Request,
        inputs: &Inputs,
        cache: Option<&mut Cache>,
    ) -> Result<(Rect, Vec<[f32; 4]>)> {
        let image = self.image(graph, source, settings, request, inputs)?;
        let region = request.region.unwrap_or(image.extent());
        Ok((region, graph.render(&image, region, cache)?))
    }

    /// `Pipeline.framed`: the scan healed, stage 0 with the lens's map, a positive read as a negative last.
    pub fn framed(&mut self, graph: &mut Graph, image: Image, s: &PipelineSettings, scale: f64, inputs: &Inputs) -> Result<Image> {
        let healed = self.healed(graph, image, s, scale, inputs)?;
        let e = healed.extent();
        // The map is laid on the full-resolution scan's shape, so every scale zooms alike.
        let lens = s.lens((f64::from(e.w) / scale, f64::from(e.h) / scale));
        let out = geometry::apply(healed, &s.geometry, lens.as_ref(), scale);
        Ok(if s.inverted_source && !s.mode.is_digital() { out.then(stages::reciprocal(conversion::INVERSION, conversion::INVERSION_FLOOR)) } else { out })
    }

    /// The scan in its own frame, before stage 0: its corrections, then the capture's exposure evened and
    /// the flat-field, so a patch takes the gains of the place it lies on and none slides as the crop moves.
    /// A digital photo was shot on no copy stand: its corrections, then its lens's vignetting where the
    /// optics darkened the scene.
    pub fn healed(&mut self, graph: &mut Graph, image: Image, s: &PipelineSettings, scale: f64, inputs: &Inputs) -> Result<Image> {
        let mut out = self.patched(graph, image, s, scale, inputs)?;
        if s.mode.is_digital() {
            return Ok(match s.optical_vignetting() {
                Some(stops) => {
                    let extent = out.extent();
                    out.then(stages::optical_vignette(extent, stops))
                }
                None => out,
            });
        }
        if s.capture_stops != 0.0 {
            out = out.then(stages::exposure(-s.capture_stops));
        }
        self.flat_field(graph, out, s, inputs)
    }

    /// A digital photo's scene on its virtual film: exposed, developed and scanned as that negative.
    pub fn filmed(&mut self, graph: &mut Graph, image: Image, s: &PipelineSettings) -> Result<Image> {
        let Some(f) = s.film() else { return Ok(image) };
        let stock = film::stock(f.stock);
        let source = match self.films.get(&f.stock) {
            Some(source) => source.clone(),
            None => {
                let (w, h) = stock.kernel.table_size();
                let source = graph.upload(w as u32, h as u32, &stock.table)?;
                self.films.insert(f.stock, source.clone());
                source
            }
        };
        Ok(image.then_reading(stages::virtual_negative(&stock.kernel_at(f.exposure)), &[Image::new(&source)]))
    }

    fn flat_field(&mut self, graph: &mut Graph, image: Image, s: &PipelineSettings, inputs: &Inputs) -> Result<Image> {
        let f = &s.flat_field;
        if f.is_neutral() {
            return Ok(image);
        }
        let (mask, peaks, per_channel) = if flat_field::shipped(f) {
            if self.valoi.is_none() {
                let m = flat_field::valoi();
                self.valoi = Some((graph.upload(m.width, m.height, &m.pixels)?, m.peaks));
            }
            let (mask, peaks) = self.valoi.as_ref().expect("uploaded above");
            (mask.clone(), *peaks, false)
        } else {
            match inputs.custom_flat_field {
                Some(c) if f.mask == FlatFieldMask::Custom && f.custom.as_deref() == Some(c.id.as_str()) => {
                    (c.mask.clone(), c.peaks, c.per_channel)
                }
                // The frame's own mask is gone: the stage is left out, as the Swift app leaves it.
                _ => return Ok(image),
            }
        };
        let fitted = flat_field::fitted(Image::new(&mask), peaks, per_channel, image.extent());
        Ok(image.then_reading(stages::flat_field(&flat_field::arguments(f, s.mode, per_channel)), &[fitted]))
    }

    /// The decoded scan with each correction's patch through its feathered stroke, laid together; one
    /// with no patch is left out. A patch is cut on it and keyed on its reading alone.
    pub fn patched(&mut self, graph: &mut Graph, image: Image, s: &PipelineSettings, scale: f64, inputs: &Inputs) -> Result<Image> {
        let patched = |c: &&oni_settings::frame::Correction| !c.stroke.is_empty();
        let named: Vec<_> = s.corrections.iter().filter(patched).filter_map(|c| Some((c, *inputs.patches.get(&c.id)?))).collect();
        self.healing.laid(graph, image, &named, scale)
    }

    fn cube(&mut self, graph: &mut Graph, name: &'static str, cube: &ColorCube) -> Result<(Image, usize)> {
        if !self.cubes.contains_key(name) {
            let source = upload_cube(graph, cube)?;
            self.cubes.insert(name, (source, cube.cube_dimension));
        }
        let (source, n) = &self.cubes[name];
        Ok((Image::new(source), *n))
    }

    /// The warmth look's move on a rendered code and its share, the code itself at a count of zero.
    fn warmed(&mut self, graph: &mut Graph, code: &Image, count: i64) -> Result<(Image, f32)> {
        let machine = &*scanner::MACHINE;
        let Some((cube, share)) = machine.warmth(count) else { return Ok((code.clone(), 0.0)) };
        let name = if count > 0 { "warmth-plus" } else { "warmth-minus" };
        let (table, n) = self.cube(graph, name, cube)?;
        Ok((code.clone().then_reading(stages::color_cube(n), &[table]), share))
    }

    /// `ScannerRender.oniFinish`: the legacy print's machine's look on ONI's render in display code, then TEMP's warmth.
    fn oni_finish(&mut self, graph: &mut Graph, image: Image, legacy: &LegacyPrint) -> Result<Image> {
        let frontier = legacy.machine == LegacyMachine::Frontier;
        // The Frontier's look stands where ONI's does, on the same render in plain display code.
        let encode = stages::scanner_encode(if frontier { 1.0 } else { oni_look::ROOT });
        let code = stages::laid(image, vec![encode]);
        let (table, n) = if frontier { self.cube(graph, "frontier-look", &frontier_look::CUBE)? } else { self.cube(graph, "oni-look", &oni_look::CUBE)? };
        let looked = code.then_reading(stages::color_cube(n), &[table]);
        let (warmth, share) = self.warmed(graph, &looked, legacy.w)?;
        Ok(looked.then_reading(stages::scanner_output(share, scanner::LIFT, 0.0), &[warmth]))
    }

    /// The legacy print's TEMP on a paper's print, its warmth look laid on the print's plain display code; the print as
    /// it is at no count.
    fn warmed_paper(&mut self, graph: &mut Graph, image: Image, legacy: &LegacyPrint) -> Result<Image> {
        if legacy.w == 0 {
            return Ok(image);
        }
        let code = stages::laid(image, vec![stages::scanner_encode(1.0)]);
        let (warmth, share) = self.warmed(graph, &code, legacy.w)?;
        Ok(code.then_reading(stages::scanner_output(share, scanner::LIFT, 0.0), &[warmth]))
    }

    fn curves(&mut self, graph: &mut Graph, image: Image, set: &CurveSet) -> Result<Image> {
        let Some(k) = finishing::curves(set) else { return Ok(image) };
        let (rgb, luma) = match self.luts.iter().find(|(s, _, _)| s == set) {
            Some((_, rgb, luma)) => (rgb.clone(), luma.clone()),
            None => {
                let n = k.rgb_lut.len() as u32;
                let rgb = graph.upload(n, 1, &k.rgb_lut)?;
                let luma = graph.upload(n, 1, &k.luma_lut)?;
                if self.luts.len() >= KEPT_LUTS {
                    self.luts.remove(0);
                }
                self.luts.push((set.clone(), rgb.clone(), luma.clone()));
                (rgb, luma)
            }
        };
        Ok(image.then_reading(stages::curves(&k), &[Image::new(&rgb), Image::new(&luma)]))
    }

    /// Curves to the output clip.
    fn finishing(&mut self, graph: &mut Graph, toned: Image, s: &PipelineSettings, scale: f64, reference: f64, measuring: bool) -> Result<Image> {
        let sheet = s.sheet.is_some();
        let mono = s.is_monochrome();
        let graded = self.curves(graph, toned, &s.manual_curves)?;
        // The spectrogram, the hand's saturation and the vignette in one pass.
        let extent = graded.extent();
        let mut run: Vec<_> = finishing::spectrogram(&s.spectrogram).map(|k| stages::spectrogram(&k)).into_iter().collect();
        run.extend(saturation(&s.saturation));
        if s.vignette.stops != 0.0 {
            run.push(stages::vignette(extent, s.vignette.stops, if mono { 0.0 } else { Print::HUNT_EXPONENT }));
        }
        let vignetted = stages::laid(graded, run);
        // One two-pass stage when Texture rests between them, else denoise and sharpening on either side.
        let joint = s.texture.amount == 0.0;
        let cleaned = if sheet { vignetted } else { detail(vignetted, finishing::detail(Some(&s.chroma), joint.then_some(&s.sharpen), scale)) };
        let textured = texture(cleaned, s, reference);
        let sharpened = if sheet || joint { textured } else { detail(textured, finishing::detail(None, Some(&s.sharpen), scale)) };
        // The sauce, the monochrome luma and the output clip in one pass.
        let mut run: Vec<_> = saturation(&finishing::sauce(s.secret_sauce.min(finishing::SAUCE_TOP))).into_iter().collect();
        // A black and white film keeps its luminance alone.
        if mono {
            run.push(stages::luma_only());
        }
        if measuring {
            return Ok(stages::laid(sharpened, run));
        }
        run.push(stages::output_clip());
        let clipped = stages::laid(sharpened, run);
        Ok(if sheet { clipped.then(OnGround) } else { clipped })
    }
}

/// The tables a render of `s` bakes at their first use, baked now: its virtual film's stock and the enlarger its
/// paper prints through, so a render asked next finds them ready.
pub fn bake_ahead(s: &PipelineSettings) {
    if let Some(f) = s.film() {
        film::stock(f.stock);
    }
    if let Some(paper) = s.paper().filter(|_| !s.print.bypassed) {
        paper::enlarger(paper::Film::of(s), paper);
    }
}

fn saturation(curve: &SaturationCurve) -> Option<stages::PerPixel> {
    finishing::saturation(curve).map(|k| stages::saturation(&k))
}

/// `Pipeline.applyDetail`: the rows pass, then the columns reading it beside the source.
fn detail(image: Image, k: Option<oni_render::Detail>) -> Image {
    let Some(k) = k else { return image };
    let rows = image.clone().then(DetailRows(k));
    image.then_reading(DetailColumns(k), &[rows])
}

/// `Pipeline.applyTexture`: three blurs and three kernels on the luma, each kernel but the last read
/// by the next blur's taps, so seven passes instead of ten.
pub fn texture(image: Image, s: &PipelineSettings, reference: f64) -> Image {
    let Some((narrow, wide, gain)) = finishing::texture(&s.texture, reference) else { return image };
    let narrowed = if narrow <= BLUR_FLOOR { image.clone().then(stages::luma_only()) } else { stages::blurred_of(image.clone(), narrow, Before::Luma) };
    // `finishing::texture` rests under `MINIMUM_BASE`, so the wide blurs always run.
    let moments = stages::blurred_of(narrowed.clone(), wide, Before::Moments);
    let based = stages::blurred_of(moments, wide, Before::Coeff(finishing::texture::EDGE_CONTRAST));
    image.then_reading(stages::texture(gain), &[narrowed, based])
}
