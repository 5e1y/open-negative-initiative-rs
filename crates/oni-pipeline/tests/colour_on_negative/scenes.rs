//! The colour checks' frames: scene patches and how each pipe reads them, the colour pipes, the prints and their
//! measures.

#![allow(dead_code)]

use crate::common::{V4, noise};
use crate::frames::*;
use oni_pipeline::look_lab::measure;
use oni_pipeline::{conversion, film};
use oni_render::{LUMA, Negative, PAPER_CURVE_NODES, Paper, Stage47, cpu};
use oni_settings::cg::Point;
use oni_settings::curves::SaturationCurve;
use oni_settings::spectrogram::Band;
use oni_settings::{ConversionMode, PaperStock, PipelineSettings, VirtualStock};

pub fn onis(s: PipelineSettings) -> PipelineSettings {
    PipelineSettings { print_paper: None, ..s }
}

pub fn slide() -> PipelineSettings {
    let mut s = PipelineSettings::rest(ConversionMode::Positive, true);
    s.gains.stops = [0.3, 0.0, -0.4];
    s
}

/// A scan noise and a virtual negative's scan.
pub fn pixels() -> Vec<V4> {
    let stock = film::stock(VirtualStock::KodakPortra800);
    let mut pixels: Vec<V4> = noise(17, 13, 7, 0.0, 1.5).iter().map(|p| cpu::virtual_negative(*p, &stock.kernel, &stock.table)).collect();
    pixels.extend(noise(17, 13, 8, 0.0, 1.0));
    pixels
}

/// Scene patches in linear Rec. 2020: greys over eight stops, then colours round the hue circle at four levels.
pub fn scene() -> Vec<V4> {
    let mut out: Vec<V4> = (0..GREYS).map(|i| 0.184 * 2f32.powf(-4.0 + 0.5 * i as f32)).map(|g| [g, g, g, 1.0]).collect();
    for level in [0.08f32, 0.3, 1.0, 2.5] {
        for h in (0..360).step_by(10) {
            let a = (h as f32).to_radians();
            let c = |o: f32| 0.184 * level * (1.0 + 0.7 * (a - o).cos());
            out.push([c(0.0), c(2.094), c(-2.094), 1.0]);
        }
    }
    out
}

pub const GREYS: usize = 17;

/// A negative shot on Portra 400 and scanned, read as Prepare the Roll reads its own reference.
pub fn shot_on() -> PipelineSettings {
    let stock = VirtualStock::KodakPortra400;
    PipelineSettings { mode: ConversionMode::Negative, ..film::on_reference(stock, &stock.reference()) }
}

/// What stage 4-7 reads of `scene` on `s`: a digital photo's virtual negative, a scanned negative shot on Portra 400, a
/// slide's own light, a slide on a paper its internegative's.
pub fn scanned(s: &PipelineSettings, scene: &[V4]) -> Vec<V4> {
    if let Some(copy) = oni_pipeline::paper::internegative::copy(s) {
        let baked = film::stock(oni_pipeline::paper::internegative::STOCK);
        return scene.iter().map(|p| cpu::virtual_negative(cpu::color_matrix(*p, &copy.light), &baked.kernel, &baked.table)).collect();
    }
    if let Some(f) = s.film() {
        let baked = film::stock(f.stock);
        let white = conversion::scene_white(s);
        return scene.iter().map(|p| cpu::virtual_negative(white.map_or(*p, |m| cpu::color_matrix(*p, &m)), &baked.kernel_at(f.exposure), &baked.table)).collect();
    }
    if s.mode.reads_positive() {
        return scene.iter().map(|p| [p[0].min(1.0), p[1].min(1.0), p[2].min(1.0), 1.0]).collect();
    }
    let baked = film::stock(VirtualStock::KodakPortra400);
    scene.iter().map(|p| cpu::virtual_negative(*p, &baked.kernel, &baked.table)).collect()
}

/// The scan pixel holding densities `d` through the frame's own gains, a slide's as its own reading takes them.
pub fn scan_of(s: &PipelineSettings, d: [f32; 3]) -> V4 {
    let pedestal = oni_settings::axis::density_axis::pedestal(conversion::rendered_ceiling(s));
    let light: [f32; 3] = std::array::from_fn(|c| 10f32.powf(-d[c]) / s.gains.stops[c].exp2() - pedestal);
    [light[0], light[1], light[2], 1.0]
}

/// What stage 4 reads of a scan pixel: a slide on a paper, its internegative's scan.
pub fn as_read(s: &PipelineSettings, q: V4) -> V4 {
    match oni_pipeline::paper::internegative::copy(s) {
        Some(copy) => {
            let baked = film::stock(oni_pipeline::paper::internegative::STOCK);
            cpu::virtual_negative(cpu::color_matrix(q, &copy.light), &baked.kernel, &baked.table)
        }
        None => q,
    }
}

/// The colour pipes, each with Print lift so a paper's print is its dyes' own light.
pub fn colour_pipes() -> Vec<(&'static str, PipelineSettings)> {
    let lifted = |mut s: PipelineSettings| {
        s.print.print_lift = true;
        s
    };
    let negative = shot_on();
    vec![
        ("ONI's print, digital", onis(digital())),
        ("ONI's print, negative", onis(negative.clone())),
        ("ONI's print, slide", slide()),
        ("paper, digital", digital()),
        ("paper, Frontier", PipelineSettings { print_paper: Some(PaperStock::FujifilmFrontierSp3000), ..digital() }),
        ("paper, negative", lifted(PipelineSettings { print_paper: Some(PaperStock::default()), ..negative.clone() })),
        ("neutral paper", lifted(PipelineSettings { print_paper: Some(PaperStock::default()), paperless: true, ..negative })),
        ("paper, slide", lifted(PipelineSettings { print_paper: Some(PaperStock::default()), ..slide() })),
    ]
}

pub fn rgb(v: V4) -> [f32; 3] {
    [v[0], v[1], v[2]]
}

pub fn printed(s: &PipelineSettings, scene: &[V4]) -> Vec<[f32; 3]> {
    let k = conversion::stage47(s);
    scanned(s, scene).iter().map(|p| rgb(cpu::stage47(*p, &k))).collect()
}

/// Oklab of a print's linear Rec. 2020 light, clipped to the screen as the render's output is.
pub fn lab(v: [f32; 3]) -> [f64; 3] {
    let m = [[1.660_49, -0.587_641, -0.072_850], [-0.124_550, 1.132_9, -0.008_349], [-0.018_150, -0.100_579, 1.118_73]];
    let v = v.map(|x| f64::from(x.clamp(0.0, 1.0)));
    measure::oklab(std::array::from_fn(|r| (0..3).map(|c| m[r][c] * v[c]).sum::<f64>().max(0.0)))
}

pub fn chroma(v: [f32; 3]) -> f64 {
    let l = lab(v);
    l[1].hypot(l[2])
}

/// The colour patches' mean Oklab chroma.
pub fn mean_chroma(print: &[[f32; 3]]) -> f64 {
    print[GREYS..].iter().map(|v| chroma(*v)).sum::<f64>() / (print.len() - GREYS) as f64
}

pub fn with(s: &PipelineSettings, f: impl Fn(&mut PipelineSettings)) -> PipelineSettings {
    let mut t = s.clone();
    f(&mut t);
    t
}

/// The former Saturation laid on a print: each channel's light to the power `2^travel`, the luma's power undone.
pub fn on_the_print(v: [f32; 3], travel: f32) -> [f32; 3] {
    let m = 2f32.powf(travel);
    let p = v.map(|c| c.max(0.0).powf(m));
    let y = (0..3).map(|c| LUMA[c] * p[c]).sum::<f32>();
    if y <= 1e-12 { v } else { p.map(|c| c * y.powf(1.0 / m - 1.0)) }
}

/// The paper's dye densities that print `light`, by Newton's steps on its log, and how far past its curves' ends they
/// lie: none for a colour its dyes print.
pub fn past_the_dyes(p: &Paper, light: [f32; 3]) -> f64 {
    let target = light.map(|v| f64::from(v.max(1e-9)).ln());
    let view = |d: [f64; 3]| p.viewed(d.map(|v| v as f32)).map(|v| f64::from(v.max(1e-12)).ln());
    let mut d = [1.0f64; 3];
    for _ in 0..60 {
        let f = view(d);
        let miss: [f64; 3] = std::array::from_fn(|c| f[c] - target[c]);
        let j: [[f64; 3]; 3] = std::array::from_fn(|r| {
            std::array::from_fn(|c| {
                let mut e = d;
                e[c] += 1e-4;
                (view(e)[r] - f[r]) / 1e-4
            })
        });
        let [[a, b, c], [dd, e, f2], [g, h, i]] = j;
        let det = a * (e * i - f2 * h) - b * (dd * i - f2 * g) + c * (dd * h - e * g);
        if det.abs() < 1e-12 {
            break;
        }
        let inv = [
            [(e * i - f2 * h) / det, (c * h - b * i) / det, (b * f2 - c * e) / det],
            [(f2 * g - dd * i) / det, (a * i - c * g) / det, (c * dd - a * f2) / det],
            [(dd * h - e * g) / det, (b * g - a * h) / det, (a * e - b * dd) / det],
        ];
        d = std::array::from_fn(|r| d[r] - (0..3).map(|c| inv[r][c] * miss[c]).sum::<f64>());
    }
    let (low, high) = (p.curves[0], p.curves[PAPER_CURVE_NODES - 1]);
    (0..3).map(|c| (f64::from(low[c].min(high[c])) - d[c]).max(d[c] - f64::from(low[c].max(high[c]))).max(0.0)).fold(0.0, f64::max)
}

/// Each layer's density as the negative's colour tools read it: the log exposure a grey develops it at, from the
/// reference grey's.
pub fn as_grey(n: &Negative, q: V4) -> [f32; 3] {
    let d = n.development;
    let read = n.read(q);
    let own: [f32; 3] = std::array::from_fn(|c| (read[c] / n.scale[c]).max(d.foot[c]).min(d.top[c]));
    let grey = d.as_grey(n.owned(d.inhibited(n.moved(d.exposures(own)))));
    std::array::from_fn(|c| grey[c] - n.pivot[c])
}

/// The hue a colour's chroma holds where the negative reads it, in degrees.
pub fn scene_hue(k: &Stage47, q: V4) -> Option<f32> {
    let about = as_grey(&k.negative?, q);
    let neutral = (0..3).map(|c| LUMA[c] * about[c]).sum::<f32>();
    let chroma = about.map(|v| v - neutral);
    let (a, b) = (chroma[0] - 0.5 * chroma[1] - 0.5 * chroma[2], 0.866_025_4 * (chroma[1] - chroma[2]));
    (a.hypot(b) > 1e-4).then(|| b.atan2(a).to_degrees().rem_euclid(360.0))
}

/// The colour tools at their tracks' ends and together, Lighting moved.
pub fn tools() -> Vec<(&'static str, fn(&mut PipelineSettings))> {
    vec![
        ("Saturation +100 %", |u| u.density = 1.0),
        ("Saturation -100 %", |u| u.density = -1.0),
        ("Red and Blue mixed", |u| {
            u.spectrogram.set(Band::Red, -20.0, 0.8, -0.6);
            u.spectrogram.set(Band::Blue, 25.0, -0.5, 0.8);
        }),
        ("curve", |u| u.saturation = SaturationCurve { points: [(0.0, 0.3), (0.25, 0.7), (0.5, 0.5), (0.75, 0.6), (1.0, 0.4)].map(|(x, y)| Point::new(x, y)).to_vec() }),
        ("curve at its ends", |u| u.saturation = SaturationCurve { points: [(0.0, 0.0), (0.2, 0.9), (0.5, 0.3), (0.8, 0.7), (1.0, 0.1)].map(|(x, y)| Point::new(x, y)).to_vec() }),
        ("all, Lighting moved", |u| {
            u.density = 0.6;
            u.spectrogram.set(Band::Green, 10.0, 0.4, 0.3);
            u.saturation = SaturationCurve { points: [(0.0, 0.2), (0.25, 0.6), (0.5, 0.5), (0.75, 0.6), (1.0, 0.3)].map(|(x, y)| Point::new(x, y)).to_vec() };
            (u.print.exposure, u.print.contrast) = (u.print.exposure + 0.7, 0.2);
            u.print.set_filters([0.2, 0.0, -0.2]);
        }),
    ]
}

/// How far a ramp's print turns back at worst, as a share of its light.
pub fn turn(y: &[f32]) -> f32 {
    y.windows(2).map(|w| (w[0] - w[1] - 1e-5).max(0.0) / w[0].abs().max(1e-3)).fold(0.0f32, f32::max)
}
