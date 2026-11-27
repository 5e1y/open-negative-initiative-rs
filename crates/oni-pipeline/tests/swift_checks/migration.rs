//! The density migration's frozen kernel against today's render, on synthetic frames and the reference roll.

use super::*;

/// A literal, so the oracle cannot follow a moving resting ceiling.
const FROZEN_PEDESTAL: f32 = 0.008;
/// Twenty times under half an 8-bit step: a drift this far shows long before a cached thumbnail would.
const FRAME_AGREEMENT: f32 = 1e-4;

/// The frozen kernel: transmittance clamped into `[Tmin, 1]`, density divided by `DMAX`, three levels
/// passes, each clamped under its black.
fn frozen(source: V3, stops: V3, levels: &LevelsSet) -> V3 {
    let lv = [&levels.red, &levels.green, &levels.blue];
    std::array::from_fn(|i| {
        let d = -((source[i] + FROZEN_PEDESTAL) * stops[i].exp2()).clamp(FROZEN_PEDESTAL / 8.0, 1.0).log10();
        ((d / pipeline::DMAX - lv[i].black) / (lv[i].white - lv[i].black).max(1e-4)).max(0.0).powf(lv[i].gamma())
    })
}

/// The windows as a sidecar of the frozen kernel's day stored them, read through the migration chain.
fn migrated(levels: &LevelsSet) -> LevelsSet {
    let stored = PipelineSettings { levels: *levels, ..PipelineSettings::default() };
    let aged = oni_settings::migrate::reshape::as_stored(&oni_settings::Document::of(stored), oni_settings::migrate::before_1_0::DENSITY_AXIS_VERSION - 1);
    oni_settings::migrate::document(&serde_json::to_vec(&aged).unwrap()).unwrap().document.settings.levels
}

struct Arm {
    name: &'static str,
    frozen: LevelsSet,
    stops: V3,
    migrating: bool,
}

fn arms() -> [Arm; 3] {
    let rest = LevelsSet { red: Levels::default(), green: Levels::default(), blue: Levels::default() };
    let at = |black, white, mid| Levels { black, white, mid, ..Levels::default() };
    let graded = LevelsSet { red: at(0.0731, 0.8137, 0.4411), green: at(0.1279, 0.7411, 0.5237), blue: at(0.2113, 0.9337, 0.6113) };
    let stops = [0.7331, 0.0, -0.4111];
    [
        Arm { name: "at rest", frozen: rest, stops: [0.0; 3], migrating: true },
        Arm { name: "graded", frozen: graded, stops, migrating: true },
        Arm { name: "the twin, left unmigrated", frozen: graded, stops, migrating: false },
    ]
}

/// `truncated(before: .curves)` with the print bypassed and the decompression whole, as the Swift arm sets it.
fn live(arm: &Arm) -> PipelineSettings {
    let mut s = PipelineSettings::default();
    s.gains.stops = arm.stops;
    s.density_ceiling = -FROZEN_PEDESTAL.log10();
    s.levels = if arm.migrating { migrated(&arm.frozen) } else { arm.frozen };
    s.vignette = Default::default();
    s.chroma = oni_settings::blocks::ChromaDenoise::OFF;
    s.spectrogram = Default::default();
    s.saturation = Default::default();
    s.secret_sauce = 0.0;
    s.texture = Default::default();
    s.sharpen.amount = 0.0;
    s.print.bypassed = true;
    s.chroma_decompression = 1.0;
    s
}

/// The decode at most `side` long, each output pixel the mean of its whole step.
fn reduced(image: &oni_decode::Linear, side: usize) -> (u32, u32, Vec<[f32; 4]>) {
    let step = image.width.max(image.height).div_ceil(side).max(1);
    let (w, h) = (image.width / step, image.height / step);
    let mut out = vec![[0.0f32; 4]; w * h];
    for oy in 0..h {
        for ox in 0..w {
            let mut s = [0.0f64; 3];
            for y in oy * step..(oy + 1) * step {
                for p in &image.pixels[y * image.width + ox * step..y * image.width + (ox + 1) * step] {
                    (0..3).for_each(|c| s[c] += p[c] as f64);
                }
            }
            let n = (step * step) as f64;
            out[oy * w + ox] = [(s[0] / n) as f32, (s[1] / n) as f32, (s[2] / n) as f32, 1.0];
        }
    }
    (w as u32, h as u32, out)
}

/// The worst post-clip gap between the frozen kernel and `rendered`.
fn gap(source: &[[f32; 4]], rendered: &[[f32; 4]], arm: &Arm) -> f32 {
    let mut against = 0.0f32;
    for (p, out) in source.iter().zip(rendered) {
        let old = frozen([p[0], p[1], p[2]], arm.stops, &arm.frozen);
        for c in 0..3 {
            against = against.max((pipeline::clip(old[c]) - pipeline::clip(out[c])).abs());
        }
    }
    against
}

#[test]
fn the_migrated_windows_render_the_frozen_kernel_on_a_real_frame() {
    let check = "the_migrated_windows_render_the_frozen_kernel_on_a_real_frame";
    let mut files: Vec<std::path::PathBuf> = oni_fixtures::raw_dir(check)
        .and_then(|d| std::fs::read_dir(d).ok())
        .map(|r| r.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|e| e == "NEF")).collect())
        .unwrap_or_default();
    files.sort();
    files.truncate(1);
    files.extend(oni_fixtures::lumix_dir(check).map(|d| d.join("P1072392.RW2")).filter(|p| p.exists()));
    let Some(mut g) = graph(check) else { return };
    for file in &files {
        let decoded = oni_decode::decode(file, 1.0).expect("decode");
        let name = file.file_name().unwrap().to_string_lossy().into_owned();
        // The thumbnail through the live chain on the GPU, the full frame through its scalar reference.
        let (w, h, small) = reduced(&decoded, 320);
        let source = g.upload(w, h, &small).expect("the thumbnail uploads");
        for arm in arms() {
            let (_, out) = Pipeline::new().render(&mut g, &source, &live(&arm), &Request::default(), &Inputs::default(), None).unwrap();
            let against = gap(&small, &out, &arm);
            println!("{name} thumbnail {w} x {h}, {}: frozen {against:.3e}", arm.name);
            if arm.migrating {
                assert!(against <= FRAME_AGREEMENT, "{name} thumbnail, {}: {against:.3e}", arm.name);
            } else {
                // Twin: windows left three times too narrow are a full-scale wrong render.
                assert!(against > 0.5, "{name}: the unmigrated twin refuses only at {against:.4}");
            }
        }
        for arm in arms().iter().filter(|a| a.migrating) {
            let k = conversion::stage47(&live(arm));
            let out: Vec<[f32; 4]> = decoded.pixels.iter().map(|&p| cpu::stage47(p, &k)).collect();
            let against = gap(&decoded.pixels, &out, arm);
            println!("{name} full {} x {}, {}: frozen {against:.3e}", decoded.width, decoded.height, arm.name);
            assert!(against <= FRAME_AGREEMENT, "{name} full, {}: {against:.3e}", arm.name);
        }
    }
}
