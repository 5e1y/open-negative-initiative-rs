//! A chart against a film's reference: the ColorChecker, Oklch rings and a grey ramp as a render prints them, grey matched,
//! and their distance to the reference's.

use crate::chart::{M3, RAMP, Render, SRGB_TO_XYZ, V3, cielab, dec, enc, inv, mul, oklab};

/// Oklch to linear sRGB.
pub fn oklch(l: f64, c: f64, h: f64) -> V3 {
    let (a, b) = (c * h.cos(), c * h.sin());
    let l_ = (l + 0.3963377774 * a + 0.2158037573 * b).powi(3);
    let m_ = (l - 0.1055613458 * a - 0.0638541728 * b).powi(3);
    let s_ = (l - 0.0894841775 * a - 1.2914855480 * b).powi(3);
    [4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_, -1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_, -0.0041960863 * l_ - 0.7034186147 * m_ + 1.7076147010 * s_]
}

pub struct Cube {
    n: usize,
    nodes: Vec<V3>,
}
impl Cube {
    pub fn read(path: &str) -> Cube {
        let (_, look) = read(&std::fs::read(path).unwrap()).unwrap();
        Cube { n: look.size, nodes: look.nodes.iter().map(|p| [p[0] as f64, p[1] as f64, p[2] as f64]).collect() }
    }
    pub fn at(&self, c: V3) -> V3 {
        let n = self.n;
        let p = c.map(|v| v.clamp(0.0, 1.0) * (n - 1) as f64);
        let i = p.map(|v| (v.floor() as usize).min(n - 2));
        let f: V3 = std::array::from_fn(|k| p[k] - i[k] as f64);
        let mut out = [0.0; 3];
        for (db, dg, dr) in (0..8).map(|q| (q >> 2 & 1, q >> 1 & 1, q & 1)) {
            let w = (if db == 1 { f[2] } else { 1.0 - f[2] }) * (if dg == 1 { f[1] } else { 1.0 - f[1] }) * (if dr == 1 { f[0] } else { 1.0 - f[0] });
            let node = self.nodes[(i[0] + dr) + n * (i[1] + dg) + n * n * (i[2] + db)];
            (0..3).for_each(|k| out[k] += w * node[k]);
        }
        out
    }
}

/// The ColorChecker's chromatic patches, first on the chart.
pub const CHECKER: usize = 18;

/// The chart's colours: the ColorChecker's chromatic patches, Oklch rings held in sRGB and the grey ramp, as display
/// codes.
pub fn colours() -> Vec<V3> {
    let cc: [V3; 18] = [
        [37.54, 14.37, 14.92], [64.66, 19.27, 17.5], [49.32, -3.82, -22.54], [43.46, -12.74, 22.72], [54.94, 9.61, -24.79], [70.48, -32.26, -0.37],
        [62.73, 35.83, 56.5], [39.43, 10.75, -45.17], [50.57, 48.64, 16.67], [30.1, 22.54, -20.87], [71.77, -24.13, 58.19], [71.51, 18.24, 67.37],
        [28.37, 15.42, -49.8], [54.38, -39.72, 32.27], [42.43, 51.05, 28.62], [81.8, 2.67, 80.41], [50.63, 51.28, -14.12], [49.57, -29.71, -28.32],
    ];
    let brad: M3 = [[0.9555766, -0.0230393, 0.0631636], [-0.0282895, 1.0099416, 0.0210077], [0.0122982, -0.0204830, 1.3299098]];
    let to_srgb = inv(&SRGB_TO_XYZ);
    let mut out: Vec<V3> = cc
        .iter()
        .map(|&[l, a, b]| {
            let fy = (l + 16.0) / 116.0;
            let f = |t: f64| if t.powi(3) > 0.008856 { t.powi(3) } else { (t - 16.0 / 116.0) / 7.787 };
            let xyz = [0.96422 * f(fy + a / 500.0), f(fy), 0.82521 * f(fy - b / 200.0)];
            mul(&to_srgb, mul(&brad, xyz)).map(enc)
        })
        .collect();
    for l in [0.5, 0.65, 0.8] {
        for c in [0.05, 0.1] {
            for h in 0..12 {
                let lin = oklch(l, c, h as f64 * std::f64::consts::TAU / 12.0);
                if lin.iter().all(|v| (0.0..=1.0).contains(v)) {
                    out.push(lin.map(enc));
                }
            }
        }
    }
    out.extend(RAMP.map(|v| [v; 3]));
    out
}

/// A display code's linear light scaled by `k` channel by channel, as Oklab.
pub fn scaled(code: V3, k: V3) -> V3 {
    oklab(std::array::from_fn(|c| enc(dec(code[c]) * k[c])))
}

/// The chart as `render` prints it, each channel's light scaled so the ramp's mid grey matches the reference's, as Oklab: a
/// render is judged on its colours, its grey's exposure and balance set as the reference's.
pub fn printed(render: &Render, chart: &[V3], cube: &Cube) -> Vec<V3> {
    let grey = [RAMP[4]; 3];
    let (want, have) = (cube.at(grey), render.code(grey));
    let k: V3 = std::array::from_fn(|c| dec(want[c]) / dec(have[c]).max(1e-9));
    chart.iter().map(|c| scaled(render.code(*c), k)).collect()
}

/// CIEDE2000 between two CIELAB colours.
pub fn de2000(p: V3, q: V3) -> f64 {
    let (rad, deg) = (std::f64::consts::PI / 180.0, 180.0 / std::f64::consts::PI);
    let c_mean = (p[1].hypot(p[2]) + q[1].hypot(q[2])) / 2.0;
    let g = 0.5 * (1.0 - (c_mean.powi(7) / (c_mean.powi(7) + 25f64.powi(7))).sqrt());
    let (a1, a2) = ((1.0 + g) * p[1], (1.0 + g) * q[1]);
    let (c1, c2) = (a1.hypot(p[2]), a2.hypot(q[2]));
    let hue = |b: f64, a: f64| (b.atan2(a) * deg).rem_euclid(360.0);
    let (h1, h2) = (hue(p[2], a1), hue(q[2], a2));
    let dh = if c1 * c2 == 0.0 { 0.0 } else if (h2 - h1).abs() <= 180.0 { h2 - h1 } else if h2 - h1 > 180.0 { h2 - h1 - 360.0 } else { h2 - h1 + 360.0 };
    let big_dh = 2.0 * (c1 * c2).sqrt() * (dh / 2.0 * rad).sin();
    let (l_mean, cp_mean) = ((p[0] + q[0]) / 2.0, (c1 + c2) / 2.0);
    let h_mean = if c1 * c2 == 0.0 { h1 + h2 } else if (h1 - h2).abs() <= 180.0 { (h1 + h2) / 2.0 } else if h1 + h2 < 360.0 { (h1 + h2 + 360.0) / 2.0 } else { (h1 + h2 - 360.0) / 2.0 };
    let t = 1.0 - 0.17 * ((h_mean - 30.0) * rad).cos() + 0.24 * (2.0 * h_mean * rad).cos() + 0.32 * ((3.0 * h_mean + 6.0) * rad).cos() - 0.20 * ((4.0 * h_mean - 63.0) * rad).cos();
    let theta = 30.0 * (-((h_mean - 275.0) / 25.0).powi(2)).exp();
    let rc = 2.0 * (cp_mean.powi(7) / (cp_mean.powi(7) + 25f64.powi(7))).sqrt();
    let sl = 1.0 + 0.015 * (l_mean - 50.0).powi(2) / (20.0 + (l_mean - 50.0).powi(2)).sqrt();
    let (sc, sh) = (1.0 + 0.045 * cp_mean, 1.0 + 0.015 * cp_mean * t);
    let rt = -(2.0 * theta * rad).sin() * rc;
    let (dl, dc, dhh) = ((q[0] - p[0]) / sl, (c2 - c1) / sc, big_dh / sh);
    (dl * dl + dc * dc + dhh * dhh + rt * dc * dhh).sqrt()
}

/// A printed chart against the reference's: mean ΔE00 and mean |ΔL*| over the colours, the ColorChecker's chroma over the
/// reference's, the ramp's mean C* and the reference's own.
pub fn judged(printed: &[V3], targets: &[V3]) -> [f64; 5] {
    let (p, t): (Vec<V3>, Vec<V3>) = (printed.iter().map(|v| cielab(*v)).collect(), targets.iter().map(|v| cielab(*v)).collect());
    let colours = targets.len() - RAMP.len();
    let mean = |f: &dyn Fn(usize) -> f64| (0..colours).map(f).sum::<f64>() / colours as f64;
    let chroma = |v: &[V3]| v[..CHECKER].iter().map(|c| c[1].hypot(c[2])).sum::<f64>();
    let ramp = |v: &[V3]| v[colours..].iter().map(|c| c[1].hypot(c[2])).sum::<f64>() / RAMP.len() as f64;
    [mean(&|i| de2000(t[i], p[i])), mean(&|i| (t[i][0] - p[i][0]).abs()), chroma(&p) / chroma(&t), ramp(&p), ramp(&t)]
}

/// A reference's 3D LUT as a `.cube` file holds it, its nodes red fastest.
struct Look {
    size: usize,
    nodes: Vec<[f32; 4]>,
}

/// The sides a cube may have: two nodes are a cube, past 128 a table outgrows a texture's side.
const SIZES: std::ops::RangeInclusive<usize> = 2..=128;

/// The title and the nodes `bytes` hold, or why they are refused: no size, a 1D table, a domain other than 0 to 1, a
/// node missing, extra or not a number.
fn read(bytes: &[u8]) -> Result<(Option<String>, Look), String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "it is not text".to_string())?;
    let (mut title, mut size, mut nodes) = (None, None, Vec::new());
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let (word, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let rest = rest.trim();
        match word {
            "TITLE" => title = Some(rest.trim_matches('"').to_string()).filter(|t| !t.is_empty()),
            "LUT_3D_SIZE" => size = Some(rest.parse::<usize>().ok().filter(|n| SIZES.contains(n)).ok_or_else(|| format!("its size \"{rest}\" is not 2 to 128"))?),
            "LUT_1D_SIZE" => return Err("it is a 1D table, not a 3D one".into()),
            "DOMAIN_MIN" | "DOMAIN_MAX" => {
                let end = if word == "DOMAIN_MIN" { 0.0 } else { 1.0 };
                if numbers(rest).ok().is_none_or(|v| v.len() != 3 || v.iter().any(|&x| x != end)) {
                    return Err(format!("its {word} is not {end} {end} {end}"));
                }
            }
            "LUT_3D_INPUT_RANGE" => return Err("its input range is not 0 to 1".into()),
            _ => {
                let v = numbers(line).map_err(|_| format!("the line \"{line}\" is neither a keyword nor a node"))?;
                let [r, g, b] = v[..] else { return Err(format!("the node \"{line}\" holds {} numbers, not 3", v.len())) };
                nodes.push([r, g, b, 1.0]);
            }
        }
    }
    let size = size.ok_or("it gives no LUT_3D_SIZE")?;
    if nodes.len() != size.pow(3) {
        return Err(format!("it holds {} nodes where a size of {size} needs {}", nodes.len(), size.pow(3)));
    }
    Ok((title, Look { size, nodes: nodes.into() }))
}

fn numbers(text: &str) -> Result<Vec<f32>, std::num::ParseFloatError> {
    text.split_whitespace().map(|w| w.parse::<f32>().and_then(|x| if x.is_finite() { Ok(x) } else { "x".parse() })).collect()
}
