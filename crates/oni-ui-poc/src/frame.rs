//! One frame of a roll: decoded, reduced for the canvas, and run through stages 4 to 7 with the
//! settings its sidecar holds.

use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use oni_render::{Stage47, cpu};
use serde_json::Value;

/// The reference roll; `ONI_RAW_DIR` points elsewhere.
pub fn roll_dir() -> PathBuf {
    oni_fixtures::folder(oni_fixtures::RAW_DIR)
}

/// The frame the proof of concept opens: Lighting exposure 1.5 EV, no contrast, the roll's levels.
pub const FRAME: &str = "DSC_6440.NEF";

/// Long side of the canvas copy, in pixels: what a 1100-point window shows at 2x.
pub const CANVAS_SIDE: usize = 1500;

/// Scene-linear Rec. 2020, row-major, alpha 1.
#[derive(Clone)]
pub struct Linear {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<[f32; 4]>,
}

/// Linear Rec. 2020 to linear sRGB, both on D65.
const REC2020_TO_SRGB: [[f32; 3]; 3] = [
    [1.660_491, -0.587_641, -0.072_850],
    [-0.124_550, 1.132_9, -0.008_349],
    [-0.018_151, -0.100_579, 1.118_73],
];

impl Linear {
    /// A box average by a whole factor, in floating point, so no channel is quantised on the way.
    pub fn reduced(&self, side: usize) -> Linear {
        let k = self.width.max(self.height).div_ceil(side).max(1);
        let (w, h) = (self.width / k, self.height / k);
        let mut pixels = Vec::with_capacity(w * h);
        let n = (k * k) as f32;
        for y in 0..h {
            for x in 0..w {
                let mut acc = [0.0f32; 4];
                for dy in 0..k {
                    let row = (y * k + dy) * self.width + x * k;
                    for p in &self.pixels[row..row + k] {
                        for c in 0..4 {
                            acc[c] += p[c];
                        }
                    }
                }
                pixels.push(acc.map(|v| v / n));
            }
        }
        Linear { width: w, height: h, pixels }
    }
}

#[derive(Debug)]
pub enum FrameError {
    Decode(String),
    Sidecar(String),
    /// A face or mode whose render is not `Pipeline.apply`'s Pro path on a colour negative.
    Unsupported(String),
}

/// The frame's own settings, read as the Swift sidecar names them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AsSet {
    /// `captureStops`, undone as `CIExposureAdjust` with EV `−captureStops` before everything.
    pub capture_stops: f32,
    pub stage47: Stage47,
}

fn at<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(v, |v, key| match key.parse::<usize>() {
        Ok(i) => v.get(i),
        Err(_) => v.get(key),
    })
}

fn number(v: &Value, path: &str) -> Result<f32, FrameError> {
    at(v, path).and_then(Value::as_f64).map(|x| x as f32).ok_or_else(|| FrameError::Sidecar(format!("{path} missing")))
}

/// A field Swift decodes with `decodeIfPresent ?? default`.
fn number_or(v: &Value, path: &str, default: f32) -> Result<f32, FrameError> {
    match at(v, path) {
        None | Some(Value::Null) => Ok(default),
        Some(x) => x.as_f64().map(|x| x as f32).ok_or_else(|| FrameError::Sidecar(format!("{path} is not a number"))),
    }
}

fn clamp(v: f32, range: RangeInclusive<f32>) -> f32 {
    v.clamp(*range.start(), *range.end())
}

/// `Pipeline.dmax`, the positive's mirror and the kernel's `window`.
const DMAX: f32 = 3.0;
/// `Levels.midRange`, `shadowRange`, `highlightRange`.
const MID_RANGE: RangeInclusive<f32> = 0.15..=0.85;
const SHADOW_RANGE: RangeInclusive<f32> = 0.05..=0.45;
const HIGHLIGHT_RANGE: RangeInclusive<f32> = 0.55..=0.95;
/// `Print.monochromeContrastRange`, `filterRange` and `monochromePointRange`, the clamps of its decoder.
const CONTRAST_RANGE: RangeInclusive<f32> = -0.5..=1.0;
const FILTER_RANGE: RangeInclusive<f32> = -1.5..=1.5;
const POINT_RANGE: RangeInclusive<f32> = -0.5..=0.5;
/// `Print.minimumSpan`.
const MINIMUM_SPAN: f32 = 0.02;
/// `Separation.range`.
const SEPARATION_RANGE: RangeInclusive<f32> = 0.0..=2.0;
/// `RollReference.factorRange`.
const FACTOR_RANGE: RangeInclusive<f32> = 0.25..=4.0;
/// `Graduation.overshoot` and `top`, `−log10(Pipeline.tmin)` with `tmin` the guard at the axis base.
const OVERSHOOT: f32 = 0.35;

fn graduation_span() -> f32 {
    let base = DMAX - 8f32.log10();
    let tmin = 10f32.powf(-base) / 8.0;
    -tmin.log10() + OVERSHOOT
}

/// One channel's window as the sidecar stores it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Window {
    black: f32,
    white: f32,
    mid: f32,
    shadows: f32,
    highlights: f32,
}

impl Window {
    fn read(s: &Value, c: &str) -> Result<Window, FrameError> {
        let f = |k: &str| number(s, &format!("levels.{c}.{k}"));
        Ok(Window { black: f("black")?, white: f("white")?, mid: f("mid")?, shadows: f("shadows")?, highlights: f("highlights")? })
    }

    /// `Levels.gamma`: the power that carries the median onto one half.
    fn gamma(&self) -> f32 {
        0.5f32.ln() / clamp(self.mid, MID_RANGE).ln()
    }

    fn shadow_ordinate(&self) -> f32 {
        0.5 - clamp(self.shadows, SHADOW_RANGE)
    }

    fn highlight_ordinate(&self) -> f32 {
        1.5 - clamp(self.highlights, HIGHLIGHT_RANGE)
    }

    /// `ChromaDecompression.signedSpan`.
    fn signed_span(&self) -> f32 {
        let span = self.white - self.black;
        if span.abs() < 1e-4 { if span < 0.0 { -1e-4 } else { 1e-4 } } else { span }
    }
}

/// `Print.heldPoints`: a sum past `1 − minimumSpan` is held back, inward moves giving way first.
fn held_points(black: f32, white: f32) -> (f32, f32) {
    let limit = 1.0 - MINIMUM_SPAN;
    if black + white <= limit {
        return (black, white);
    }
    if black > 0.0 && white > 0.0 {
        let share = limit / (black + white);
        return (black * share, white * share);
    }
    if black > white { (limit - white, white) } else { (black, limit - black) }
}

/// `Separation.Family.dyeRows`, output channel per row.
fn dye_rows(family: &str) -> Option<[[f32; 3]; 3]> {
    Some(match family {
        "kodakC41" => [[1.0000, -0.2379, -0.0092], [-0.0346, 1.0000, -0.0136], [-0.0327, -0.0790, 1.0000]],
        "fujiC41" => [[1.0000, -0.3034, -0.0059], [-0.0688, 1.0000, -0.0138], [-0.0129, -0.0655, 1.0000]],
        "genericC41" => [[1.0000, -0.2706, -0.0076], [-0.0517, 1.0000, -0.0137], [-0.0228, -0.0723, 1.0000]],
        "ecn2" => [[1.0000, -0.2706, -0.0082], [-0.0995, 1.0000, -0.0252], [0.0260, -0.1023, 1.0000]],
        "e6" => [[1.0000, -0.3584, 0.0067], [-0.3109, 1.0000, -0.0588], [-0.0269, -0.2385, 1.0000]],
        _ => return None,
    })
}

type M3 = [[f32; 3]; 3];
const IDENTITY: M3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

fn mul(m: &M3, v: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| m[i][0] * v[0] + m[i][1] * v[1] + m[i][2] * v[2])
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `Separation.rows` for a negative: `chromaKept(matrix(family, at: dose))`, each row normalised to sum to one.
fn separation_rows(family: &str, strength: f32) -> Result<M3, FrameError> {
    let rows = dye_rows(family).ok_or_else(|| FrameError::Sidecar(format!("separationFamily {family} unknown")))?;
    let full: M3 = rows.map(|r| {
        let sum = r[0] + r[1] + r[2];
        r.map(|x| x / sum)
    });
    let s = clamp(strength, SEPARATION_RANGE);
    let m: M3 = std::array::from_fn(|i| std::array::from_fn(|j| IDENTITY[i][j] + s * (full[i][j] - IDENTITY[i][j])));
    // `chromaGain`: the square root of the determinant on the plane every grey is orthogonal to.
    let u = [1.0 / 2f32.sqrt(), -1.0 / 2f32.sqrt(), 0.0];
    let v = [1.0 / 6f32.sqrt(), 1.0 / 6f32.sqrt(), -2.0 / 6f32.sqrt()];
    let (a, b, c, d) = (dot(u, mul(&m, u)), dot(u, mul(&m, v)), dot(v, mul(&m, u)), dot(v, mul(&m, v)));
    let gain = (a * d - b * c).abs().sqrt();
    if m == IDENTITY || gain <= 1e-6 {
        return Ok(m);
    }
    Ok(m.map(|r| r.map(|x| x * (1.0 / gain) + (1.0 / 3.0) * (1.0 - 1.0 / gain))))
}

/// The Pro face's stage 4–7 arguments for a colour negative, as `Pipeline.apply` passes them.
pub fn as_set(sidecar: &Value) -> Result<AsSet, FrameError> {
    let s = &sidecar["settings"];
    let text = |path: &str, default: &str| -> Result<String, FrameError> {
        match at(s, path) {
            None | Some(Value::Null) => Ok(default.to_owned()),
            Some(Value::String(t)) => Ok(t.clone()),
            Some(_) => Err(FrameError::Sidecar(format!("{path} is not a string"))),
        }
    };
    let face = text("face", "pro")?;
    if face != "pro" {
        return Err(FrameError::Unsupported(format!("face {face}: the Scanner face swaps the conversion")));
    }
    let mode = at(s, "mode").and_then(Value::as_str).ok_or_else(|| FrameError::Sidecar("mode missing".into()))?;
    if mode != "negative" {
        return Err(FrameError::Unsupported(format!("mode {mode}")));
    }

    let ceiling = number(s, "densityCeiling")?;
    let pedestal = 10f32.powf(-ceiling);
    let gain = [number(s, "gains.stops.0")?, number(s, "gains.stops.1")?, number(s, "gains.stops.2")?];
    let windows = [Window::read(s, "red")?, Window::read(s, "green")?, Window::read(s, "blue")?];

    let exposure = number_or(s, "print.exposure", 0.0)?;
    let contrast = clamp(number_or(s, "print.contrast", 0.0)?, CONTRAST_RANGE);
    let zones = [number_or(s, "print.shadows", 0.0)?, number_or(s, "print.highlights", 0.0)?];
    let own = [
        clamp(number_or(s, "print.red.exposure", 0.0)?, FILTER_RANGE),
        clamp(number_or(s, "print.green.exposure", 0.0)?, FILTER_RANGE),
        clamp(number_or(s, "print.blue.exposure", 0.0)?, FILTER_RANGE),
    ];
    let hunt_base = match at(s, "print.huntBase") {
        None | Some(Value::Null) => None,
        Some(b) => Some((
            number_or(b, "exposure", 0.0)?,
            number_or(b, "contrast", 0.0)?,
            [number_or(b, "shadows", 0.0)?, number_or(b, "highlights", 0.0)?],
        )),
    };
    let (base_stops, base_contrast, base_zones) = hunt_base.unwrap_or((0.0, 0.0, [0.0, 0.0]));
    let black_point = clamp(number_or(s, "print.blackPoint", 0.0)?, POINT_RANGE);
    let white_point = number_or(s, "print.whitePoint", 0.0)?;
    let (held_black, held_white) = held_points(black_point, white_point);

    // `Print.reaching`: each window end moved by its share of the span, the balance keeping the stored ends.
    let reached = windows.map(|w| {
        let span = w.white - w.black;
        if black_point == 0.0 && white_point == 0.0 {
            [w.black, w.white, w.gamma()]
        } else {
            [w.black + held_black * span, w.white - held_white * span, w.gamma()]
        }
    });

    let dose = if at(s, "rgbPasses").is_some_and(|v| !v.is_null()) { 0.0 } else { number(s, "separation")? };
    let family = text("separationFamily", "genericC41")?;
    let sep = separation_rows(&family, dose)?;

    // `ChromaDecompression.weights`: a referenced frame scales the windows' colour by the span ratio.
    let (decompression, film_share) = match at(s, "rollReference") {
        None | Some(Value::Null) => {
            let dose = number(s, "chromaDecompression")?.clamp(0.0, 1.0);
            (dose, 1.0 - dose)
        }
        Some(r) => {
            let span = graduation_span();
            let reached = (1.0 - held_black - held_white) / (1.0 - 0.0);
            let mut product = 1.0f32;
            for (c, w) in windows.iter().enumerate() {
                let roll = (span * (number(r, &format!("white.{c}"))? - number(r, &format!("black.{c}"))?)).abs();
                product *= (w.white - w.black).abs().max(1e-4) / roll.max(1e-4) * reached.max(1e-4);
            }
            (clamp(product.powf(1.0 / 3.0), FACTOR_RANGE), 0.0)
        }
    };

    Ok(AsSet {
        capture_stops: number(s, "captureStops")?,
        stage47: Stage47 {
            gain: gain.map(f32::exp2),
            red: reached[0],
            green: reached[1],
            blue: reached[2],
            invert: 1.0,
            density: 2f32.powf(number(s, "density")?),
            pedestal,
            log_guard: pedestal / 8.0,
            window: DMAX,
            low_points: windows.map(|w| w.shadow_ordinate()),
            high_points: windows.map(|w| w.highlight_ordinate()),
            sep_r: sep[0],
            sep_g: sep[1],
            sep_b: sep[2],
            print_on: 1.0,
            print_stops: exposure,
            print_contrast: contrast,
            print_zones: zones,
            own_stops: own,
            hunt: 1.0,
            base_stops,
            base_contrast,
            base_zones,
            decompression,
            balance_low: windows.map(|w| w.black),
            balance_span: windows.map(|w| w.signed_span()),
            film_share,
            follow: 0.0,
        },
    })
}

pub struct Frame {
    pub path: PathBuf,
    pub canvas: Linear,
    pub set: AsSet,
}

impl Frame {
    pub fn open(path: &Path) -> Result<Frame, FrameError> {
        let sidecar_path = PathBuf::from(format!("{}.json", path.display()));
        let text = std::fs::read_to_string(&sidecar_path).map_err(|e| FrameError::Sidecar(e.to_string()))?;
        let sidecar: Value = serde_json::from_str(&text).map_err(|e| FrameError::Sidecar(e.to_string()))?;
        let set = as_set(&sidecar)?;
        let decoded = oni_decode::decode(path, 1.0).map_err(|e| FrameError::Decode(format!("{e:?}")))?;
        let full = Linear { width: decoded.width, height: decoded.height, pixels: decoded.pixels };
        Ok(Frame { path: path.to_owned(), canvas: full.reduced(CANVAS_SIDE), set })
    }

    /// Stages 4 to 7 with `stage47` on the canvas copy.
    pub fn converted(&self, stage47: &Stage47) -> Linear {
        converted(&self.canvas, self.set.capture_stops, stage47)
    }
}

/// `captureStops` undone, then stages 4 to 7 with `stage47`, on every core, linear light out.
pub fn converted(negative: &Linear, capture_stops: f32, stage47: &Stage47) -> Linear {
    let lift = (-capture_stops).exp2();
    let mut pixels = negative.pixels.clone();
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let chunk = pixels.len().div_ceil(workers).max(1);
    std::thread::scope(|scope| {
        for part in pixels.chunks_mut(chunk) {
            scope.spawn(move || {
                for p in part {
                    let lifted = [p[0] * lift, p[1] * lift, p[2] * lift, p[3]];
                    *p = cpu::stage47(lifted, stage47);
                }
            });
        }
    });
    Linear { pixels, ..*negative }
}

/// The sRGB transfer function.
fn encode(v: f32) -> f32 {
    if v <= 0.003_130_8 { 12.92 * v } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
}

/// Linear Rec. 2020 to sRGB, hard-clipped as the output clip is, quantised only here at the end.
pub fn display_rgba8(image: &Linear) -> Vec<u8> {
    let m = REC2020_TO_SRGB;
    image
        .pixels
        .iter()
        .flat_map(|p| {
            let t = |r: usize| (m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2]).clamp(0.0, 1.0);
            let q = |v: f32| (encode(v) * 255.0).round() as u8;
            [q(t(0)), q(t(1)), q(t(2)), 255]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sidecar() -> Value {
        serde_json::from_str(
            r#"{"settings": {"face": "pro", "mode": "negative", "captureStops": -0.5, "densityCeiling": 3.5,
                "gains": {"stops": [-1, 0, 1]}, "density": 0, "separation": 0.6, "separationFamily": "kodakC41",
                "chromaDecompression": 0.5, "rollReference": null,
                "print": {"exposure": 1.5, "contrast": 2, "red": {"exposure": 0.25}},
                "levels": {"red": {"black": 0.8, "white": 2.0, "mid": 0.5, "shadows": 0.25, "highlights": 0.75},
                           "green": {"black": 0.7, "white": 2.1, "mid": 0.4, "shadows": 0.3, "highlights": 0.75},
                           "blue": {"black": 0.6, "white": 2.4, "mid": 0.5, "shadows": 0.25, "highlights": 0.75}}}}"#,
        )
        .unwrap()
    }

    #[test]
    fn the_sidecar_maps_onto_the_kernel_arguments() {
        let doc = sidecar();
        let k = as_set(&doc).unwrap().stage47;
        assert_eq!(k.gain, [0.5, 1.0, 2.0]);
        assert_eq!(k.blue, [0.6, 2.4, 1.0]);
        assert!((k.pedestal - 10f32.powf(-3.5)).abs() < 1e-9);
        assert_eq!((k.print_on, k.print_stops, k.hunt, k.follow, k.base_stops), (1.0, 1.5, 1.0, 0.0, 0.0));
        // The decoder's clamps: Contrast to −0.5…1; absent filters at 0.
        assert_eq!((k.print_contrast, k.own_stops), (1.0, [0.25, 0.0, 0.0]));
        // A median of 0.4 brightens: gamma ln ½ / ln 0.4, and the shadow handle moves its ordinate.
        assert!((k.green[2] - 0.5f32.ln() / 0.4f32.ln()).abs() < 1e-6 && k.green[2] < 1.0);
        assert!((k.low_points[1] - 0.2).abs() < 1e-6 && k.low_points[0] == 0.25);
        assert_eq!((k.decompression, k.film_share), (0.5, 0.5));
        // Twin: a sidecar missing a window field is refused, never read as zero.
        let mut broken = doc.clone();
        broken["settings"]["levels"]["red"].as_object_mut().unwrap().remove("white");
        assert!(as_set(&broken).is_err());
        // Twin: the Scanner face converts through its own table, so it is refused rather than misread.
        let mut scanner = doc.clone();
        scanner["settings"]["face"] = "scanner".into();
        assert!(matches!(as_set(&scanner), Err(FrameError::Unsupported(_))));
    }

    #[test]
    fn the_separation_keeps_greys_and_the_mean_saturation() {
        let rows = separation_rows("kodakC41", 0.6).unwrap();
        for r in rows {
            assert!((r[0] + r[1] + r[2] - 1.0).abs() < 1e-6, "a grey is a fixed point: {r:?}");
        }
        assert_eq!(separation_rows("kodakC41", 0.0).unwrap(), IDENTITY);
        // Twin: the bare dose, without `chromaKept`, multiplies the chroma and differs.
        let full = dye_rows("kodakC41").unwrap().map(|r| r.map(|x| x / (r[0] + r[1] + r[2])));
        let bare: M3 = std::array::from_fn(|i| std::array::from_fn(|j| IDENTITY[i][j] + 0.6 * (full[i][j] - IDENTITY[i][j])));
        assert!((0..3).any(|i| (0..3).any(|j| (bare[i][j] - rows[i][j]).abs() > 1e-3)));
    }

    #[test]
    fn the_points_hold_a_minimum_span() {
        assert_eq!(held_points(0.1, 0.2), (0.1, 0.2));
        let (b, w) = held_points(0.6, 0.6);
        assert!((b + w - (1.0 - MINIMUM_SPAN)).abs() < 1e-6 && (b - w).abs() < 1e-6);
        assert_ne!((b, w), (0.6, 0.6), "twin: the unheld points would cross the ends");
        assert_eq!(held_points(-0.3, 1.5), (-0.3, 1.0 - MINIMUM_SPAN + 0.3));
    }

    #[test]
    fn the_reduction_averages_in_floating_point() {
        let src = Linear { width: 4, height: 2, pixels: [[0.1, 0.2, 0.3, 1.0], [0.3, 0.2, 0.1, 1.0], [1e-4, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0, 1.0]].repeat(2) };
        let out = src.reduced(2);
        assert_eq!((out.width, out.height), (2, 1));
        assert!((out.pixels[0][0] - 0.2).abs() < 1e-7);
        // A value no 8-bit buffer holds survives the reduction.
        assert!((out.pixels[1][0] - 5e-5).abs() < 1e-9);
        assert!(out.pixels[1][0] > 0.0 && (out.pixels[1][0] * 255.0) < 0.5, "twin: quantised to 8 bits it would be 0");
    }
}
