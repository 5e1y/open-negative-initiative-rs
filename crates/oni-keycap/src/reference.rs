//! The render against the Swift bench's own sheet, `keycap-lab/exports/variantes.png` (`KeycapLab
//! --shot`): its six keys rebuilt here from the bench's style and layout, field by field.

use crate::style::{KeycapStyle, Rgba};
use crate::surface::{Frame, SIGMAS, Sigmas, compose_with};

/// `Shot.magnification`: the sheet is drawn at six pixels per point.
const SCALE: f64 = 6.0;
/// `Color(white: 0.32)`, the sheet's ground, as stored.
const GROUND: f32 = 82.0 / 255.0;

/// The sheet's `KeycapStyle()`: a 2 pt contour, a white backing, the reference's light, no midtone match.
pub fn bench_style() -> KeycapStyle {
    KeycapStyle {
        contour_width: 2.0,
        ambient: 0.5,
        diffuse: 0.5,
        light_azimuth: 75.0,
        midtone_match: 0.0,
        shadow_blur: 16.0,
        shadow_y: 16.0,
        shadow_opacity: 0.6,
        ..KeycapStyle::default()
    }
}

/// The sheet's `pressAmbientLoss`: a pressed cap loses 0.1 of ambient at full depth.
fn at_depth(style: &KeycapStyle, depth: f64) -> KeycapStyle {
    KeycapStyle { ambient: (style.ambient - 0.1 * depth).max(0.0), ..style.clone() }
}

/// One key of the sheet: its style, depth, size and top-left corner, in points.
pub struct Specimen {
    pub style: KeycapStyle,
    pub depth: f64,
    pub size: (f64, f64),
    pub at: (f64, f64),
}

/// The sheet's six keys: `padding(28)`, rows 24 apart, keys 28 apart, the small row centred.
pub fn specimens() -> Vec<Specimen> {
    let large = bench_style();
    let small = large.scaled(0.7);
    let mut out = Vec::new();
    for (i, depth) in [0.0, 0.25, 1.0].into_iter().enumerate() {
        out.push(Specimen { style: at_depth(&large, depth), depth, size: (100.0, 70.0), at: (28.0 + 128.0 * i as f64, 28.0) });
        out.push(Specimen { style: at_depth(&small, depth), depth, size: (78.0, 54.0), at: (61.0 + 106.0 * i as f64, 140.0) });
    }
    out
}

pub struct Reference {
    pub width: usize,
    pub height: usize,
    pub rgb: Vec<f32>,
}

pub fn load() -> Option<Reference> {
    let dir = std::env::var_os("ONI_KEYCAP_LAB")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::Path::new(&h).join("Documents/keycap-lab")))?;
    let file = std::fs::File::open(dir.join("exports/variantes.png")).ok()?;
    let mut reader = png::Decoder::new(std::io::BufReader::new(file)).read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let step = info.color_type.samples();
    let rgb = buf[..info.buffer_size()].chunks_exact(step).flat_map(|p| [p[0], p[1], p[2]]).map(|v| v as f32 / 255.0).collect();
    Some(Reference { width: info.width as usize, height: info.height as usize, rgb })
}

/// The comparison window around a key, in points: left, top, and below down to its name.
const AROUND: (f64, f64, f64) = (8.0, 8.0, 9.0);

/// The six keys drawn on the ground at their places, as RGB floats the size of the reference;
/// neighbours' cast shadows reach each other's windows, as on the sheet.
pub fn sheet(reference: &Reference, sigmas: Sigmas, wrong: &impl Fn(&mut Specimen)) -> Vec<f32> {
    let mut rgb = vec![GROUND; reference.width * reference.height * 3];
    for mut spec in specimens() {
        wrong(&mut spec);
        let margin = (3.0 * sigmas.shadow * spec.style.shadow_blur + spec.style.shadow_y).ceil() + 1.0;
        let frame = Frame { width: spec.size.0, height: spec.size.1, scale: SCALE, margin };
        let plane = compose_with(&spec.style, frame, spec.depth, Some(Rgba::white(1.0)), sigmas);
        let (left, top) = (((spec.at.0 - margin) * SCALE) as isize, ((spec.at.1 - margin) * SCALE) as isize);
        for y in 0..plane.height {
            for x in 0..plane.width {
                let (rx, ry) = (left + x as isize, top + y as isize);
                if rx < 0 || ry < 0 || rx >= reference.width as isize || ry >= reference.height as isize {
                    continue;
                }
                let px = &plane.data[(y * plane.width + x) * 4..(y * plane.width + x) * 4 + 4];
                let at = (ry as usize * reference.width + rx as usize) * 3;
                for ch in 0..3 {
                    rgb[at + ch] = px[ch] + rgb[at + ch] * (1.0 - px[3]);
                }
            }
        }
    }
    rgb
}

/// A key's window in reference pixels: left, top, width, height.
fn window(spec: &Specimen) -> (usize, usize, usize, usize) {
    let x0 = ((spec.at.0 - AROUND.0) * SCALE) as usize;
    let y0 = ((spec.at.1 - AROUND.1) * SCALE) as usize;
    let w = ((spec.size.0 + 2.0 * AROUND.0) * SCALE) as usize;
    let h = ((spec.size.1 + AROUND.1 + AROUND.2) * SCALE) as usize;
    (x0, y0, w, h)
}

/// Whether a window pixel lies on the legend, which is text and not the chrome.
fn on_legend(spec: &Specimen, x: usize, y: usize) -> bool {
    let (px, py) = (x as f64 / SCALE - AROUND.0, y as f64 / SCALE - AROUND.1);
    let s = spec.style.scale_at(spec.depth);
    let cx = spec.size.0 / 2.0;
    let cy = spec.size.1 / 2.0 + (spec.style.label_centring() + spec.style.travel_at(spec.depth)) * s;
    (px - cx).abs() < 26.0 && (py - cy).abs() < 11.0
}

/// Mean and maximum absolute error per channel, in 8-bit levels, over every key's window.
pub fn error(reference: &Reference, sigmas: Sigmas, wrong: impl Fn(&mut Specimen)) -> (f64, f64) {
    let (mean, max, _) = errors(reference, sigmas, wrong);
    (mean, max)
}

/// As `error`, with the 99th percentile.
pub fn errors(reference: &Reference, sigmas: Sigmas, wrong: impl Fn(&mut Specimen)) -> (f64, f64, f64) {
    let ours = sheet(reference, sigmas, &wrong);
    let (mut sum, mut count, mut max) = (0.0f64, 0usize, 0.0f64);
    let mut histogram = [0usize; 256];
    for spec in specimens() {
        let (x0, y0, w, h) = window(&spec);
        for y in 0..h {
            for x in 0..w {
                if on_legend(&spec, x, y) {
                    continue;
                }
                let at = ((y0 + y) * reference.width + x0 + x) * 3;
                for ch in 0..3 {
                    let e = (ours[at + ch] - reference.rgb[at + ch]).abs() as f64 * 255.0;
                    sum += e;
                    max = max.max(e);
                    histogram[(e.round() as usize).min(255)] += 1;
                    count += 1;
                }
            }
        }
    }
    let mut seen = 0;
    let p99 = histogram.iter().position(|&n| {
        seen += n;
        seen * 100 >= count * 99
    });
    (sum / count as f64, max, p99.unwrap_or(255) as f64)
}

#[test]
fn the_render_matches_the_swift_bench() {
    let Some(reference) = load() else {
        eprintln!("keycap-lab/exports/variantes.png is absent: ONI_KEYCAP_LAB points at the bench");
        return;
    };
    let (mean, max, p99) = errors(&reference, SIGMAS, |_| {});
    eprintln!("against variantes.png: mean {mean:.2} levels, 99th percentile {p99:.0}, max {max:.0}");
    assert!(mean < MEAN_LIMIT, "mean error {mean:.2} levels");
}

#[test]
fn twin_a_wrong_light_is_seen() {
    let Some(reference) = load() else { return };
    let (mean, _) = error(&reference, SIGMAS, |s| s.style.light_azimuth += 30.0);
    eprintln!("an azimuth 30° off: mean {mean:.2}");
    assert!(mean >= MEAN_LIMIT, "an azimuth 30° off still passes at {mean:.2}");
}

#[test]
fn twin_a_wrong_blur_is_seen() {
    let Some(reference) = load() else { return };
    let (mean, _) = error(&reference, Sigmas { blur: SIGMAS.blur * 2.5, shadow: SIGMAS.shadow * 2.5 }, |_| {});
    eprintln!("blurs 2.5 times too wide: mean {mean:.2}");
    assert!(mean >= MEAN_LIMIT, "blurs 2.5 times too wide still pass at {mean:.2}");
}

/// Measured 0.91 levels; the legend and the sheet's names are left out of every window.
const MEAN_LIMIT: f64 = 1.5;

/// Sweeps the two blur conversions; `cargo test -p oni-keycap sweep -- --ignored --nocapture`.
#[test]
#[ignore]
fn sweep() {
    let reference = load().expect("the bench's exports");
    for blur in [0.5, 0.8, 0.9, 1.0] {
        for shadow in [0.5, 0.8, 0.9, 1.0] {
            let (mean, max) = error(&reference, Sigmas { blur, shadow }, |_| {});
            eprintln!("blur {blur:.2} shadow {shadow:.2}: mean {mean:.3} max {max:.0}");
        }
    }
}

/// Writes each key's window, ours beside the bench's and their difference ×4, to `ONI_KEYCAP_DUMP`.
#[test]
#[ignore]
fn dump() {
    let reference = load().expect("the bench's exports");
    let path = std::env::var("ONI_KEYCAP_DUMP").expect("ONI_KEYCAP_DUMP names the PNG");
    let ours = sheet(&reference, SIGMAS, &|_| {});
    let windows: Vec<_> = specimens().iter().map(window).collect();
    let width = windows.iter().map(|w| w.2 * 3).max().unwrap();
    let height: usize = windows.iter().map(|w| w.3).sum();
    let mut out = vec![0u8; width * height * 3];
    let mut top = 0;
    for (x0, y0, w, h) in windows {
        for y in 0..h {
            for x in 0..w {
                let at = ((y0 + y) * reference.width + x0 + x) * 3;
                for ch in 0..3 {
                    let (a, b) = (ours[at + ch], reference.rgb[at + ch]);
                    let row = (top + y) * width;
                    out[(row + x) * 3 + ch] = (a * 255.0).round() as u8;
                    out[(row + w + x) * 3 + ch] = (b * 255.0).round() as u8;
                    out[(row + 2 * w + x) * 3 + ch] = (128.0 + (a - b) * 255.0 * 4.0).clamp(0.0, 255.0) as u8;
                }
            }
        }
        top += h;
    }
    let file = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    let mut encoder = png::Encoder::new(file, width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.write_header().unwrap().write_image_data(&out).unwrap();
}
