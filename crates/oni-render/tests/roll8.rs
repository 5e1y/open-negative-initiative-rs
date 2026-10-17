//! Every frame of roll #8 at its own sidecar, through the kernels its settings run: the Pro chain
//! (capture exposure, stages 4 to 7, detail, output clip, histogram) and the Frontier's.

mod common;

use common::{each, gpu, num, pair, sidecar_stage47, Frontier, Judge};
use oni_render::gpu::Gpu;
use oni_render::*;
use serde_json::Value;

/// The NEFs roll #8 holds; fewer means a file went missing and the roll is not the reference.
const FRAMES: usize = 10;

/// `Graduation.window` for a negative at the resting ceiling, where the guard puts the top at 3.
const WINDOW: (f32, f32) = (-0.35, 3.0);

/// `ScannerRender.windowScale` and `Print.minimumSpan`.
const WINDOW_SCALE: f32 = 1.3;
const MINIMUM_SPAN: f32 = 0.02;

/// A region of the frame, its origin in Core Image's frame so coordinate kernels see where it sits.
fn region(frame: &oni_decode::Linear, x0: usize, y0: usize, w: usize, h: usize) -> Image {
    let pixels = (y0..y0 + h).flat_map(|y| (x0..x0 + w).map(move |x| frame.pixels[y * frame.width + x])).collect();
    Image { origin: [x0 as f32, (frame.height - y0 - h) as f32], ..Image::new(w, h, pixels) }
}

/// Share of the pixels whose green sits strictly inside (`low`, `high`).
fn inside(image: &Image, low: f32, high: f32) -> f32 {
    image.pixels.iter().filter(|p| p[1] > low && p[1] < high).count() as f32 / image.pixels.len() as f32
}

fn frames(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .expect("roll #8")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "NEF"))
        .collect();
    paths.sort();
    paths
}

fn sidecar(path: &std::path::Path) -> Value {
    let sidecar: Value = serde_json::from_slice(&std::fs::read(path.with_extension("NEF.json")).expect("sidecar")).expect("sidecar JSON");
    sidecar["settings"].clone()
}

/// The Pro chain at the sidecar, each stage judged on the reference's own input so errors never pile up.
fn pro(g: &Gpu, judge: &mut Judge, name: &str, negative: &Image, s: &Value) -> f32 {
    let lifted = ExposureAdjust { ev: -num(s, "captureStops") };
    let exposed = each(negative, |p| cpu::exposure_adjust(p, &lifted));
    let twin = each(negative, |p| cpu::exposure_adjust(p, &ExposureAdjust { ev: lifted.ev + 0.01 }));
    judge.image(&format!("{name} CIExposureAdjust {:+.3} EV", lifted.ev), &g.exposure_adjust(negative, &lifted), &exposed, &twin);

    let k = sidecar_stage47(s);
    let shifted = Stage47 { gain: [k.gain[0], k.gain[1] * 1.01, k.gain[2]], ..k };
    let positive = each(&exposed, |p| cpu::stage47(p, &k));
    let out = exposed.with_pixels(g.stage47(&exposed.pixels, &k));
    judge.image(&format!("{name} stage47 at {} print stops", k.print_stops), &out, &positive, &each(&exposed, |p| cpu::stage47(p, &shifted)));

    let chroma = (num(s, "chroma.radius") as f64, num(s, "chroma.force"));
    let sharpen = (num(s, "sharpen.radius") as f64, num(s, "sharpen.amount"), num(s, "sharpen.halo"));
    let k = Detail::dosed(Some(chroma), Some(sharpen)).expect("roll #8 doses its detail");
    let detailed = cpu::detail(&positive, &k);
    let wrong = Detail { sigma: k.sigma.map(|v| v * 1.05), ..k };
    judge.image(&format!("{name} detail"), &g.detail(&positive, &k), &detailed, &cpu::detail(&positive, &wrong));

    let clipped = each(&detailed, cpu::output_clip);
    // The twin's ceiling sits under the region's own brightest channel, so any region can see it.
    let top = detailed.pixels.iter().flat_map(|p| [p[0], p[1], p[2]]).fold(0f32, f32::max).min(1.0);
    let ceiling = 0.9 * top;
    let twin = each(&detailed, |p| [p[0].clamp(0.0, ceiling), p[1].clamp(0.0, ceiling), p[2].clamp(0.0, ceiling), p[3]]);
    judge.image(&format!("{name} outputClip"), &g.output_clip(&detailed), &clipped, &twin);

    let bins = 1024;
    let rect = Rect { x: 0, y: 0, width: clipped.width, height: clipped.height };
    let n = (rect.width * rect.height) as f64;
    let shares = g.area_histogram(&clipped, rect, &AreaHistogram { count: bins, scale: 1.0 });
    let counts: Vec<[u64; 4]> = shares.iter().map(|c| c.map(|v| (v as f64 * n).round() as u64)).collect();
    let reference = cpu::histogram_counts(&clipped, rect, bins);
    let nudged = each(&clipped, |p| p.map(|v| v + 0.5 / bins as f32));
    let twin = cpu::histogram_counts(&nudged, rect, bins);
    let wrong = counts.iter().zip(&reference).filter(|(a, b)| a != b).count();
    let twin_wrong = counts.iter().zip(&twin).filter(|(a, b)| a != b).count();
    println!("{name} CIAreaHistogram: {wrong} bins differ, twin {twin_wrong}");
    if wrong != 0 || twin_wrong == 0 {
        judge.fail(format!("{name} CIAreaHistogram: {wrong} bins differ, twin {twin_wrong}"));
    }
    inside(&positive, 0.02, 0.98)
}

/// The Frontier at the sidecar's clicks, Contrast and TEMP; the separation rows are of the C-41
/// family's shape, not its measured matrix, which this crate does not build.
fn scanner(g: &Gpu, judge: &mut Judge, name: &str, negative: &Image, s: &Value, f: &Frontier) -> f32 {
    let click = |k: &str| num(s, &format!("scanner.{k}")) as i64;
    let offset = f.offset([("D", click("d")), ("C", click("c")), ("M", click("m")), ("Y", click("y"))]);
    let neutral = Frontier::v3(&f.json["neutral"]["density"]);
    let range: Vec<f32> = (0..2).map(|i| f.json["render"]["densityRange"][i].as_f64().unwrap() as f32).collect();
    let pedestal = 10f32.powf(-num(s, "densityCeiling"));
    let median = WINDOW.0 + num(s, "rollReference.median.1") * (WINDOW.1 - WINDOW.0);
    let span = (num(s, "levels.green.white") - num(s, "levels.green.black")).max(MINIMUM_SPAN);
    let k = ScannerDensity {
        gain: std::array::from_fn(|c| num(s, &format!("gains.stops.{c}")).exp2()),
        pedestal,
        log_guard: pedestal / 8.0,
        median: [median; 3],
        sep_r: [1.18, -0.12, -0.06],
        sep_g: [-0.09, 1.21, -0.12],
        sep_b: [-0.04, -0.17, 1.21],
        scale: WINDOW_SCALE / span,
        shift: std::array::from_fn(|c| neutral[c] + offset[c]),
        reached_low: Frontier::v3(&f.json["render"]["reached"][0]),
        reached_high: Frontier::v3(&f.json["render"]["reached"][1]),
        low: range[0],
        span: range[1] - range[0],
        invert: 1.0,
        dmax: 3.0,
    };
    let lifted = ExposureAdjust { ev: -num(s, "captureStops") };
    let exposed = each(negative, |p| cpu::exposure_adjust(p, &lifted));
    let density = each(&exposed, |p| cpu::scanner_density(p, &k));
    let wrong = ScannerDensity { shift: k.shift.map(|v| v + 0.01), ..k };
    judge.image(&format!("{name} scannerDensity"), &g.scanner_density(&exposed, &k), &density, &each(&exposed, |p| cpu::scanner_density(p, &wrong)));

    let mut nudged = f.render.clone();
    nudged.cube_data.iter_mut().for_each(|v| v[1] += 0.002);
    let code = each(&density, |p| cpu::color_cube(p, &f.render));
    judge.image(&format!("{name} CIColorCube Frontier"), &g.color_cube(&density, &f.render), &code, &each(&density, |p| cpu::color_cube(p, &nudged)));

    // `ScannerRender.warmed`: the look of TEMP's sign, at its count's share; none at zero.
    let temp = click("w");
    let look = if temp > 0 { ("warmthPlus", &f.warmth_plus) } else { ("warmthMinus", &f.warmth_minus) };
    let per_count = f.json["looks"][look.0]["perCount"].as_f64().unwrap() as f32;
    let warmth = if temp == 0 { code.clone() } else { each(&code, |p| cpu::color_cube(p, look.1)) };
    if temp != 0 {
        judge.image(&format!("{name} CIColorCube {}", look.0), &g.color_cube(&code, look.1), &warmth, &code);
    }
    let k = ScannerOutput { warmth_share: temp.unsigned_abs() as f32 / per_count, lift: SCANNER_LIFT, contrast: num(s, "scannerFinish.contrast") };
    let wrong = ScannerOutput { contrast: k.contrast + 0.02, warmth_share: k.warmth_share + 0.02, ..k };
    let reference = pair(&code, &warmth, |c, w| cpu::scanner_output(c, w, &k));
    let twin = pair(&code, &warmth, |c, w| cpu::scanner_output(c, w, &wrong));
    judge.image(&format!("{name} scannerOutput TEMP {temp}, Contrast {}", k.contrast), &g.scanner_output(&code, &warmth, &k), &reference, &twin);
    inside(&density, 0.0, 1.0)
}

#[test]
fn every_frame_of_roll8() {
    let Some(dir) = oni_fixtures::raw_dir("every_frame_of_roll8") else { return };
    let Some(f) = common::frontier("every_frame_of_roll8") else { return };
    let Some(g) = gpu("every_frame_of_roll8") else { return };
    let mut paths = frames(&dir);
    assert_eq!(paths.len(), FRAMES, "roll #8 at {} holds {} NEFs", dir.display(), paths.len());
    // Two frames by default keep the check quick; ONI_FULL_ROLL=1 runs all ten.
    if std::env::var_os("ONI_FULL_ROLL").is_none() {
        paths.retain(|p| p.ends_with("DSC_6432.NEF") || p.ends_with("DSC_6448.NEF"));
    }
    let mut judge = Judge::default();
    let mut coverage = Vec::new();
    for path in &paths {
        let file = path.file_name().unwrap().to_string_lossy().into_owned();
        let s = sidecar(path);
        let started = std::time::Instant::now();
        let frame = oni_decode::decode(path, 1.0).unwrap_or_else(|e| panic!("{file} does not decode: {e:?}"));
        println!("{file}: {}x{} decoded in {:.1} s, face {}", frame.width, frame.height, started.elapsed().as_secs_f32(), s["face"]);
        // The centre, and the top-left corner where every neighbourhood reaches past the edge.
        let (w, h) = (768, 512);
        let centre = region(&frame, (frame.width - w) / 2, (frame.height - h) / 2, w, h);
        let corner = region(&frame, 0, 0, 384, 256);
        for (part, image) in [("centre", &centre), ("corner", &corner)] {
            let name = format!("{file} {part}");
            let printed = pro(g, &mut judge, &name, image, &s);
            let scanned = scanner(g, &mut judge, &name, image, &s, &f);
            println!("{name}: {:.1} % of the positive's green in (0.02, 0.98), {:.1} % of the density in the cube", printed * 100.0, scanned * 100.0);
            if part == "centre" {
                coverage.push(printed.min(scanned));
            }
        }
    }
    // Across the centres, most of the picture must sit where every branch is live; one dark frame may not.
    let mean = coverage.iter().sum::<f32>() / coverage.len() as f32;
    if mean < 0.5 {
        judge.fail(format!("the centres sit {:.1} % inside the range on average, the checks say little", mean * 100.0));
    }
    judge.done();
}
