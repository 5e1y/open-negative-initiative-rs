//! The readings themselves, on the render graph: the frame rendered up to a stage, then read back
//! whole for the blocks and the pipette, or reduced to `READ_SIDE` for the balance's means.

use crate::blocks::{self, Reading};
use crate::filter_balance::READ_SIDE;
use crate::plane::Plane;
use crate::truncate::{Stage, truncated};
use crate::window::{integral, intersection, measurement_windows, union};
use oni_graph::stages::{Axis, Crop, Lanczos};
use oni_graph::{Graph, Image, Rect, Source};
use oni_pipeline::resident::Resident;
use oni_pipeline::{Inputs, Pipeline, Request};
use oni_settings::cg;
use oni_settings::PipelineSettings;

/// `Negative.measureSide`: the long side of the copy every measurement reads.
pub const MEASURE_SIDE: f64 = 1400.0;

/// A frame's measuring copy on the graph, and the full-resolution width its crop is quantised on.
#[derive(Clone, Copy)]
pub struct Copy<'s> {
    pub source: &'s Source,
    pub full_width: Option<f64>,
}

pub struct Reader<'a> {
    pub graph: &'a mut Graph,
    pub pipeline: &'a mut Pipeline,
    /// The custom mask and dust patches the frame names, as `Pipeline.framed` lays them.
    pub laying: Laying<'a>,
}

/// Where a reading takes the frame's named images from.
pub enum Laying<'a> {
    /// None laid: the bare frame.
    Nothing,
    /// Read from their files onto the reader's graph.
    Read(&'a mut Resident),
    /// Already on the reader's graph, as the canvas's engine keeps them for the same frame.
    Held(Inputs<'a>),
}

/// The images `s` names, those on files read first; the pipeline lays each only where `s` names it.
fn laid<'l>(laying: &'l mut Laying<'_>, graph: &mut Graph, s: &PipelineSettings) -> Inputs<'l> {
    match laying {
        Laying::Nothing => Inputs::default(),
        Laying::Read(r) => {
            r.read(graph, s);
            r.inputs(s)
        }
        Laying::Held(held) => Inputs { patches: held.patches.clone(), custom_flat_field: held.custom_flat_field },
    }
}

/// A Core Image rectangle over `extent`'s own lower-left origin, as the graph's y-down pixels.
pub fn on_graph(extent: Rect, r: &cg::Rect) -> Rect {
    let top = f64::from(extent.h) - (r.y + r.height);
    Rect::new(extent.x + r.x as i32, extent.y + top as i32, r.width as u32, r.height as u32)
}

impl Reader<'_> {
    /// `Pipeline.measured`: the recipe in front of `before`, output clip and ground left off.
    pub fn image(&mut self, copy: Copy, s: &PipelineSettings, before: Stage) -> Option<Image> {
        let request = Request { full_width: copy.full_width, measuring: true, ..Request::default() };
        let t = truncated(s, before);
        let inputs = laid(&mut self.laying, self.graph, &t);
        self.pipeline.image(self.graph, copy.source, &t, &request, &inputs).ok()
    }

    /// `Pipeline.framed`: the head of the chain, with the mask and patches the frame names.
    pub fn framed(&mut self, source: &Source, s: &PipelineSettings, scale: f64) -> Option<Image> {
        let inputs = laid(&mut self.laying, self.graph, s);
        self.pipeline.framed(self.graph, Image::new(source), s, scale, &inputs).ok()
    }


    /// The recipe rendered whole and read back.
    pub fn plane(&mut self, copy: Copy, s: &PipelineSettings, before: Stage) -> Option<Plane> {
        let image = self.image(copy, s, before)?;
        let e = image.extent();
        if e.is_empty() {
            return None;
        }
        let pixels = self.graph.render(&image, e, None).ok()?;
        Some(Plane::new(e.w as usize, e.h as usize, pixels))
    }

    /// `Pipeline.reading`: the one door every balance reader takes, through the frame's windows.
    pub fn reading(&mut self, copy: Copy, s: &PipelineSettings, before: Stage) -> Reading {
        let Some(read) = self.plane(copy, s, before).map(Plane::counted) else { return Reading::default() };
        let mut windows = measurement_windows(&read.extent(), s);
        if windows.is_empty() {
            windows = vec![read.extent()];
        }
        blocks::reading(&read, &windows, blocks::BINS)
    }

    /// `FilterBalance.pixels`: the render through its windows at `READ_SIDE`, one linear pixel each,
    /// transparent ones left out, since a sheet's gutters are no light of it.
    pub fn balance_pixels(&mut self, copy: Copy, s: &PipelineSettings) -> Option<Vec<[f32; 3]>> {
        let image = self.image(copy, s, Stage::ManualCurves)?;
        let e = image.extent();
        let extent = cg::Rect::new(0.0, 0.0, f64::from(e.w), f64::from(e.h));
        let windows = measurement_windows(&extent, s);
        let bounds = windows.iter().skip(1).fold(windows.first().copied().unwrap_or(extent), |a, b| union(&a, b));
        let region = integral(&intersection(&bounds, &extent));
        if region.width < 1.0 || region.height < 1.0 {
            return None;
        }
        let scale = (READ_SIDE / region.width.max(region.height)).min(1.0);
        let w = ((region.width * scale).floor() as u32).max(1);
        let h = ((region.height * scale).floor() as u32).max(1);
        let crop = on_graph(e, &region);
        let mut reduced = image.then(Crop(crop));
        if scale < 1.0 {
            reduced = reduced
                .then(Lanczos { axis: Axis::X, origin: crop.x, from: crop.w, to: w })
                .then(Lanczos { axis: Axis::Y, origin: crop.y, from: crop.h, to: h });
        }
        let at = reduced.extent();
        let raw = self.graph.render(&reduced, at, None).ok()?;
        Some(raw.into_iter().filter(|p| p[3] > 0.5).map(|p| [p[0], p[1], p[2]]).collect())
    }

    /// `RawDecode.measuring(from:)`: a preview reduced to the measuring side, its edge rebuilt.
    pub fn measuring(&mut self, preview: &Source) -> Option<Source> {
        let e = preview.extent();
        let side = f64::from(e.w.max(e.h));
        if side <= MEASURE_SIDE {
            return Some(preview.clone());
        }
        let reduced = Image::new(preview).lanczos(MEASURE_SIDE / side);
        self.graph.materialise(&oni_pipeline::stages::rebuilt_edge(reduced)).ok()
    }
}
