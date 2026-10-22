//! Decoding a frame: the RAW engine, a linear scan, a plain file, RGB passes laid as one, and the reduced preview.

use super::*;

/// The editor's RAWs decoded whole as they open (the Settings' choice); off, at LibRaw's half size.
pub static FULL_DECODE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// `Negative.previewSide`: the long side every editor render and thumbnail starts from.
pub const PREVIEW_SIDE: usize = 2000;

/// Scene-linear Rec. 2020, row-major, alpha 1.
#[derive(Clone)]
pub struct Linear {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<[f32; 4]>,
}

/// A file read on the CPU: the whole frame, or a linear scan already reduced while reading.
pub struct Decoded {
    pub linear: Linear,
    /// The frame's full-resolution width, which the dosed radii and the crop are measured on.
    pub full_width: f64,
    /// `Some(grey)` for a file naming no profile, which the hand may say how to read.
    pub undeclared: Option<bool>,
}

/// How a frame's sidecar asks a file outside the RAW engine to be read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reading {
    /// Its windows were measured on its channels converted to the working primaries.
    pub primaries_converted: bool,
    /// The encoding the hand chose for a file naming no profile.
    pub source_encoding: Option<oni_settings::SourceEncoding>,
    /// The three passes a lead names, decoded and laid as one frame.
    pub passes: Option<oni_settings::frame::RgbPasses>,
}

impl Reading {
    pub fn of(s: &oni_settings::PipelineSettings) -> Reading {
        Reading { primaries_converted: s.primaries_converted, source_encoding: s.source_encoding, passes: s.rgb_passes.clone() }
    }
}

/// `RawDecode.linear` on its preview: a lead's three passes composed, any other file alone.
pub fn decode(path: &Path, reading: Reading) -> Result<Decoded, String> {
    match reading.passes.clone() {
        Some(passes) => composed(path, &passes, |p| alone(p, reading.clone())),
        None => alone(path, reading),
    }
}

/// `RawDecode.alone` on its preview: the branch the bytes claim, the preview side asked of it; a RAW
/// at LibRaw's half size, or whole when the Settings ask it, so the full resolution costs no second decode.
pub(crate) fn alone(path: &Path, reading: Reading) -> Result<Decoded, String> {
    match oni_decode::route(path) {
        oni_decode::Branch::Mosaic => mosaic(path, if FULL_DECODE.load(std::sync::atomic::Ordering::Relaxed) { 0 } else { oni_decode::HALF }),
        oni_decode::Branch::LinearRgb => {
            let (full, _) = oni_decode::container::pixel_size(path).ok_or("its container is unreadable")?;
            linear_scan(path, Some(PREVIEW_SIDE as f64), reading, full)
        }
        oni_decode::Branch::Plain => plain(path, Some(PREVIEW_SIDE as f64), reading),
    }
}

/// `decode` for a copy no longer than `side`: a RAW is read at half size when its sensor is twice as
/// long, which skips the demosaic; the measuring copies and the roll's previews never need more.
pub fn decode_for(path: &Path, reading: Reading, side: usize) -> Result<Decoded, String> {
    let one = |p: &Path, r: Reading| match oni_decode::route(p) {
        oni_decode::Branch::Mosaic => mosaic(p, side),
        _ => alone(p, r),
    };
    match reading.passes.clone() {
        Some(passes) => composed(path, &passes, |p| one(p, reading.clone())),
        None => one(path, reading),
    }
}

/// The frame's own pixels, unreduced: what a zoom past the preview reads.
pub fn decode_full(path: &Path, reading: Reading) -> Result<Decoded, String> {
    let one = |p: &Path, r: Reading| match oni_decode::route(p) {
        oni_decode::Branch::LinearRgb => linear_scan(p, None, r, 0),
        oni_decode::Branch::Plain => plain(p, None, r),
        oni_decode::Branch::Mosaic => mosaic(p, 0),
    };
    match reading.passes.clone() {
        Some(passes) => composed(path, &passes, |p| one(p, reading.clone())),
        None => one(path, reading),
    }
}

/// A RAW through LibRaw, at half size when `side` (0: whole) fits in half the sensor.
fn mosaic(path: &Path, side: usize) -> Result<Decoded, String> {
    let _slot = crate::decode_gate::slot();
    let (d, factor) = oni_decode::decode_for(path, 1.0, side).map_err(|e| format!("LibRaw cannot decode it (code {})", e.0))?;
    let full_width = (d.width * factor) as f64;
    Ok(Decoded { linear: Linear { width: d.width, height: d.height, pixels: d.pixels }, full_width, undeclared: None })
}

/// `RawDecode.alone(longestSide:)`: one file, never its passes, no longer than `side` (`None`: whole).
pub fn decode_alone(path: &Path, side: Option<f64>) -> Result<Decoded, String> {
    let reading = Reading::default();
    let mut d = match oni_decode::route(path) {
        oni_decode::Branch::Mosaic => mosaic(path, side.map_or(0, |s| s.ceil() as usize))?,
        oni_decode::Branch::LinearRgb => {
            let (full, _) = oni_decode::container::pixel_size(path).ok_or("its container is unreadable")?;
            linear_scan(path, side, reading, full)?
        }
        oni_decode::Branch::Plain => plain(path, side, reading)?,
    };
    if let Some(side) = side {
        d.linear = shrunk(&d.linear, side);
    }
    Ok(d)
}

/// The image averaged down to `side` on its long side, each pixel the mean of the source pixels
/// whose centres it covers; a smaller one is kept.
pub fn shrunk(l: &Linear, side: f64) -> Linear {
    let long = l.width.max(l.height) as f64;
    if long <= side || l.width == 0 || l.height == 0 {
        return l.clone();
    }
    let scale = side / long;
    let (w, h) = (((l.width as f64 * scale).round() as usize).max(1), ((l.height as f64 * scale).round() as usize).max(1));
    let span = |i: usize, n: usize, of: usize| {
        let (a, b) = (i * of / n, ((i + 1) * of / n).max(i * of / n + 1));
        a..b.min(of)
    };
    let mut pixels = Vec::with_capacity(w * h);
    for row in 0..h {
        let rows = span(row, h, l.height);
        for col in 0..w {
            let cols = span(col, w, l.width);
            let mut sum = [0.0f64; 4];
            for y in rows.clone() {
                for x in cols.clone() {
                    let p = l.pixels[y * l.width + x];
                    for c in 0..4 {
                        sum[c] += f64::from(p[c]);
                    }
                }
            }
            let n = (rows.len() * cols.len()).max(1) as f64;
            pixels.push(sum.map(|v| (v / n) as f32));
        }
    }
    Linear { width: w, height: h, pixels }
}

/// `RGBPasses.composed`: red from the red pass, green from the green, blue from the blue, on the
/// green pass's extent, the red and blue laid by their registration; a missing pass fails the frame.
pub(crate) fn composed(lead: &Path, passes: &oni_settings::frame::RgbPasses, decode: impl Fn(&Path) -> Result<Decoded, String>) -> Result<Decoded, String> {
    let beside = |name: &str| lead.parent().map_or_else(|| PathBuf::from(name), |f| f.join(name));
    let green = decode(&beside(&passes.green)).map_err(|e| format!("its green pass: {e}"))?;
    let red = decode(&beside(&passes.red)).map_err(|e| format!("its red pass: {e}"))?;
    let blue = decode(&beside(&passes.blue)).map_err(|e| format!("its blue pass: {e}"))?;
    let fits = passes.registration.clone().unwrap_or_default();
    let (w, h) = (green.linear.width, green.linear.height);
    let r = laid(&red.linear, &fits.red, w, h, 0);
    let b = laid(&blue.linear, &fits.blue, w, h, 2);
    let pixels = green.linear.pixels.iter().enumerate().map(|(i, g)| [r[i], g[1], b[i], 1.0]).collect();
    Ok(Decoded { linear: Linear { width: w, height: h, pixels }, full_width: green.full_width, undeclared: None })
}

/// One channel of a pass on a `w` × `h` frame through its fit, sampled bilinearly with the pass's
/// edge held past it, as `clampedToExtent().transformed(by:)` reads it; y up as Core Image counts.
pub(crate) fn laid(pass: &Linear, fit: &oni_settings::frame::Fit, w: usize, h: usize, channel: usize) -> Vec<f32> {
    let identity = *fit == oni_settings::frame::Fit::default();
    if identity && (pass.width, pass.height) == (w, h) {
        return pass.pixels.iter().map(|p| p[channel]).collect();
    }
    let (fw, fh) = (w as f64, h as f64);
    let (mx, my) = (fw / 2.0, fh / 2.0);
    let (cs, sn) = (fit.rotation.cos() * fit.scale, fit.rotation.sin() * fit.scale);
    let (a, b, c, d) = (cs, sn, -sn, cs);
    let tx = mx - (a * mx + c * my) + fit.dx * fw;
    let ty = my - (b * mx + d * my) + fit.dy * fw;
    let det = a * d - b * c;
    let texel = |x: i64, y_up: i64| -> f32 {
        let col = x.clamp(0, pass.width as i64 - 1) as usize;
        let row = (pass.height as i64 - 1 - y_up).clamp(0, pass.height as i64 - 1) as usize;
        pass.pixels[row * pass.width + col][channel]
    };
    let mut out = Vec::with_capacity(w * h);
    for row in 0..h {
        let oy = fh - row as f64 - 0.5;
        for col in 0..w {
            let (px, py) = (col as f64 + 0.5 - tx, oy - ty);
            // The fit's inverse: where in the pass this output point was taken from.
            let (ix, iy) = ((d * px - c * py) / det - 0.5, (-b * px + a * py) / det - 0.5);
            let (x0, y0) = (ix.floor(), iy.floor());
            let (fx, fy) = ((ix - x0) as f32, (iy - y0) as f32);
            let (x0, y0) = (x0 as i64, y0 as i64);
            let top = texel(x0, y0) * (1.0 - fx) + texel(x0 + 1, y0) * fx;
            let up = texel(x0, y0 + 1) * (1.0 - fx) + texel(x0 + 1, y0 + 1) * fx;
            out.push(top * (1.0 - fy) + up * fy);
        }
    }
    out
}

/// The container's own channels, or converted from its ColorMatrix1 when the windows were measured
/// so; a file naming no matrix is in the working space already. `full` 0 reads the decode's width.
pub(crate) fn linear_scan(path: &Path, side: Option<f64>, reading: Reading, full: usize) -> Result<Decoded, String> {
    let _slot = crate::decode_gate::slot();
    let mut d = oni_decode::container::linear(path, side).ok_or("its samples are unreadable")?;
    if reading.primaries_converted {
        let (matrix, illuminant) = oni_decode::container::calibration(path).ok_or("its container is unreadable")?;
        if let Some(m) = oni_colour::source::calibrated_to_working(&matrix, illuminant) {
            for p in &mut d.pixels {
                let v = oni_colour::pcs::mul_f32(&m, [p[0], p[1], p[2]]);
                p[..3].copy_from_slice(&v);
            }
        }
    }
    let full_width = if full > 0 { full } else { d.width } as f64;
    Ok(Decoded { linear: Linear { width: d.width, height: d.height, pixels: d.pixels }, full_width, undeclared: None })
}

/// `plainLinear`: the file's curve undone in its profile, else the chosen encoding, else sRGB.
pub(crate) fn plain(path: &Path, side: Option<f64>, reading: Reading) -> Result<Decoded, String> {
    let _slot = crate::decode_gate::slot();
    let mut e = oni_decode::plain::read(path, side).ok_or("no image reader takes it")?;
    oni_colour::source::Source::of(e.icc.as_deref(), reading.source_encoding, e.grey).linearise(&mut e.pixels, !reading.primaries_converted);
    let undeclared = e.icc.is_none().then_some(e.grey);
    Ok(Decoded { linear: Linear { width: e.width, height: e.height, pixels: e.pixels }, full_width: e.full.0 as f64, undeclared })
}
