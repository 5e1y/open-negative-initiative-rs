//! The export, the proof and the zebra measured on real positives: every frame of roll #8, the
//! reference roll, converted by its own sidecar. `ONI_RAW_DIR` points at another folder of RAWs.

mod common;

use std::path::{Path, PathBuf};

use lcms2::{Flags, Intent, PixelFormat, Profile, Transform};
use moxcms::{ColorProfile, ProfileClass};
use oni_colour::working::rec2020_encode;
use oni_colour::{Display, OutputProfile, Proof, Quantised, Space, gamut, legs};
use oni_render::{Stage47, cpu};
use serde_json::Value;

/// Every eighth pixel each way: a frame's distribution at a sixty-fourth of its cost.
const STEP: usize = 8;

fn raws(dir: &Path) -> Vec<PathBuf> {
    let mut raws: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|d| d.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    raws.retain(|p| p.extension().is_some_and(|e| ["nef", "rw2"].contains(&e.to_string_lossy().to_lowercase().as_str())));
    raws.sort();
    raws
}

/// The sidecar's stage 4 to 7 as the fused kernel takes them: gains, density axis, levels and
/// the print's exposure. Decompression, separation and Hunt rest, so this is a fixture's positive.
fn conversion(sidecar: &Value) -> Stage47 {
    let s = &sidecar["settings"];
    let f = |v: &Value| v.as_f64().expect("a number in the sidecar") as f32;
    let stops: Vec<f32> = s["gains"]["stops"].as_array().unwrap().iter().map(f).collect();
    let pedestal = 10f32.powf(-f(&s["densityCeiling"]));
    let level = |c: &str| {
        let l = &s["levels"][c];
        let gamma = 0.5f32.ln() / f(&l["mid"]).clamp(0.15, 0.85).ln();
        ([f(&l["black"]), f(&l["white"]), gamma], f(&l["shadows"]), f(&l["highlights"]))
    };
    let (r, g, b) = (level("red"), level("green"), level("blue"));
    let exposure = f(&s["print"]["exposure"]);
    assert_eq!(s["mode"], "negative");
    Stage47 {
        gain: std::array::from_fn(|c| stops[c].exp2()),
        red: r.0,
        green: g.0,
        blue: b.0,
        pedestal,
        log_guard: pedestal / 8.0,
        low_points: [r.1, g.1, b.1],
        high_points: [r.2, g.2, b.2],
        print_on: 1.0,
        print_stops: exposure,
        print_contrast: f(&s["print"]["contrast"]),
        base_stops: exposure,
        base_contrast: f(&s["print"]["contrast"]),
        ..Stage47::rest()
    }
}

/// The frame decoded, subsampled, converted by its sidecar, then the output clip.
fn positive(raw: &Path) -> (usize, usize, Vec<f32>) {
    let sidecar: Value = serde_json::from_slice(&std::fs::read(format!("{}.json", raw.display())).expect("each frame has its sidecar")).unwrap();
    let k = conversion(&sidecar);
    let even = (-sidecar["settings"]["captureStops"].as_f64().unwrap_or(0.0) as f32).exp2();
    let decoded = oni_decode::decode(raw, 1.0).expect("roll #8 decodes");
    let (w, h) = (decoded.width / STEP, decoded.height / STEP);
    let mut px = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            let s = decoded.pixels[y * STEP * decoded.width + x * STEP];
            let out = cpu::stage47([s[0] * even, s[1] * even, s[2] * even, 1.0], &k);
            px.extend([out[0], out[1], out[2]].map(|v| v.clamp(0.0, 1.0)));
            px.push(1.0);
        }
    }
    (w, h, px)
}

fn percentile(mut v: Vec<f32>, p: f32) -> f32 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() - 1) as f32 * p) as usize]
}

/// Per pixel, the largest channel gap, read in the cube's encoding or as it is.
fn gaps(a: &[f32], b: &[f32], encode: bool) -> Vec<f32> {
    let e = |v: f32| if encode { rec2020_encode(v.clamp(0.0, 1.0)) } else { v };
    a.chunks_exact(4).zip(b.chunks_exact(4)).map(|(x, y)| (0..3).map(|c| (e(x[c]) - e(y[c])).abs()).fold(0.0, f32::max)).collect()
}

fn share(flags: impl Iterator<Item = bool>) -> f32 {
    let (mut n, mut hit) = (0usize, 0usize);
    for f in flags {
        n += 1;
        hit += f as usize;
    }
    hit as f32 / n.max(1) as f32
}

fn striped_share(px: &[f32], width: usize, display: &Display) -> f32 {
    let mut out = px.to_vec();
    gamut::striped(&mut out, width, display, 0.0);
    share(gaps(px, &out, false).into_iter().map(|g| g > 1e-3))
}

#[test]
fn every_frame_of_roll_8_exports_proofs_and_stripes_as_measured() {
    let check = "every_frame_of_roll_8_exports_proofs_and_stripes_as_measured";
    let Some(folder) = oni_fixtures::raw_dir(check) else { return };
    let raws = raws(&folder);
    assert!(!raws.is_empty(), "no NEF or RW2 in {}", folder.display());
    let dir = common::sandbox("roll8");
    let file = |name: &str, mut p: ColorProfile| {
        p.profile_class = ProfileClass::DisplayDevice;
        let path = dir.join(name);
        std::fs::write(&path, p.encode().unwrap()).unwrap();
        OutputProfile::new(path.to_string_lossy())
    };
    let (srgb, p3) = (file("sRGB.icc", ColorProfile::new_srgb()), file("P3.icc", ColorProfile::new_display_p3()));
    // A bluish paper whose colorimetric tags let lcms2 read its black, as a real paper's do.
    let mut tables = common::stand_in_tables(0.03, false, 17, 33);
    tables.extend(tables.clone().into_iter().map(|(sig, t)| (if sig == "A2B0" { "A2B1" } else { "B2A1" }, t)));
    let paper = dir.join("Pearl.icc");
    std::fs::write(&paper, common::assemble("Pearl", 2, false, Some(common::bluish()), tables)).unwrap();
    let data = std::fs::read(&paper).unwrap();
    let (l, back) = (legs::legs(&data).unwrap(), legs::legs_back(&data).unwrap());
    let space = Space::Profile(OutputProfile::new(paper.to_string_lossy()));
    let working = Profile::new_icc(&oni_colour::working::working_profile().encode().unwrap()).unwrap();
    let theirs = Profile::new_icc(&data).unwrap();
    let lcms = |flags: Flags| -> Transform<[f32; 3], [f32; 3]> {
        Transform::new_flags(&working, PixelFormat::RGB_FLT, &theirs, PixelFormat::RGB_FLT, Intent::RelativeColorimetric, flags).unwrap()
    };
    let (with_bpc, without) = (lcms(Flags::NO_OPTIMIZE | Flags::BLACKPOINT_COMPENSATION), lcms(Flags::NO_OPTIMIZE));
    let (srgb_screen, p3_screen) = (Display::new(&ColorProfile::new_srgb()).unwrap(), Display::new(&ColorProfile::new_display_p3()).unwrap());
    let gap16 = |a: &[u16], b: &[u16]| a.iter().zip(b).map(|(x, y)| (*x as i32 - *y as i32).abs()).max().unwrap();

    // Every frame holds the agreements and their twins; the roll as a whole must reach the clip.
    let mut worst = [0.0f32; 7];
    let mut least = [f32::MAX; 3];
    let mut reached = [0.0f32; 3];
    for raw in &raws {
        let (w, h, px) = positive(raw);
        assert_eq!(px.len(), w * h * 4);
        let rows = |top: usize, n: usize, out: &mut [f32]| out.copy_from_slice(&px[top * w * 4..(top + n) * w * 4]);

        // The export through an sRGB profile file lands on the sRGB space, pixel for pixel.
        let reference: Vec<u16> = Space::Srgb.encoded(&px).unwrap().iter().map(|v| (v * 65535.0).round() as u16).collect();
        let Some(Quantised::Sixteen(through)) = srgb.converted(w, h, 16, rows) else { panic!() };
        let Some(Quantised::Sixteen(other)) = p3.converted(w, h, 16, rows) else { panic!() };
        let export = gap16(&reference, &through) as f32;
        // Twin: through Display P3 the same frame moves.
        let export_twin = gap16(&reference, &other) as f32;

        // The proof into sRGB moves only what the sRGB file clips, and moves some of it.
        let to_srgb = Space::Srgb.from_working().unwrap();
        let outside: Vec<bool> = px.chunks_exact(4).map(|p| oni_colour::pcs::mul_f32(&to_srgb, [p[0], p[1], p[2]]).iter().any(|v| !(-1e-5..=1.0 + 1e-5).contains(v))).collect();
        let moved = gaps(&px, &Proof::new(Space::Srgb).proofed(&px).unwrap(), false);
        let held = moved.iter().zip(&outside).filter(|(_, o)| !**o).map(|(g, _)| *g).fold(0.0, f32::max);
        let clipped = share(outside.iter().copied());

        // The export to the paper against lcms2's own compensation, on the same pixels.
        let device = legs::run(&l, &px).unwrap();
        let rgb: Vec<[f32; 3]> = px.chunks_exact(4).map(|p| [p[0], p[1], p[2]]).collect();
        let mut theirs_bpc = vec![[0.0f32; 3]; rgb.len()];
        with_bpc.transform_pixels(&rgb, &mut theirs_bpc);
        let mut theirs_bare = vec![[0.0f32; 3]; rgb.len()];
        without.transform_pixels(&rgb, &mut theirs_bare);
        let flat = |v: Vec<[f32; 3]>| -> Vec<f32> { v.into_iter().flat_map(|p| [p[0].clamp(0.0, 1.0), p[1].clamp(0.0, 1.0), p[2].clamp(0.0, 1.0), 1.0]).collect() };
        let oracle = gaps(&device, &flat(theirs_bpc), false);
        let (oracle_p99, oracle_max) = (percentile(oracle.clone(), 0.99), percentile(oracle, 1.0));
        // Twin: lcms2 without compensation piles the shadows on the paper's black.
        let oracle_twin = percentile(gaps(&device, &flat(theirs_bare), false), 0.99);

        // The paper proof's cube against the exact legs on the same pixels.
        let exact = legs::run_back(&back, &device, None).unwrap();
        let cube = gaps(&exact, &Proof::new(space.clone()).proofed(&px).unwrap(), true);
        let (cube_p99, cube_max) = (percentile(cube.clone(), 0.99), percentile(cube, 1.0));
        // Twin: colour only, a proof showing another white and black, sits well past it.
        let only = Proof { colour_only: true, ..Proof::new(space.clone()) };
        let cube_twin = percentile(gaps(&exact, &only.proofed(&px).unwrap(), true), 0.99);

        // Proofed in Display P3, the export's rest: a P3 screen stripes nothing, sRGB what it clips.
        let shown = Proof::new(Space::DisplayP3).proofed(&px).unwrap();
        let on_p3 = striped_share(&shown, w, &p3_screen);
        let on_srgb = striped_share(&shown, w, &srgb_screen);
        // Twin: the unproofed picture carries colours past P3, which the P3 screen then stripes.
        let unproofed = striped_share(&px, w, &p3_screen);

        println!(
            "{}: {w}×{h}, export {export} (twin {export_twin}) of 65535; past sRGB {:.2} %, kept within {held:.1e}; \
             paper vs lcms2 p99 {oracle_p99:.4} worst {oracle_max:.4} (twin {oracle_twin:.4}); cube p99 {cube_p99:.4} \
             worst {cube_max:.4} (twin {cube_twin:.4}); stripes P3 {:.2} % sRGB {:.2} % (unproofed on P3 {:.2} %)",
            raw.file_name().unwrap().to_string_lossy(),
            clipped * 100.0,
            on_p3 * 100.0,
            on_srgb * 100.0,
            unproofed * 100.0
        );
        for (slot, v) in worst.iter_mut().zip([export, held, oracle_p99, oracle_max, cube_p99, cube_max, on_p3]) {
            *slot = slot.max(v);
        }
        for (slot, v) in least.iter_mut().zip([export_twin, oracle_twin, cube_twin]) {
            *slot = slot.min(v);
        }
        for (slot, v) in reached.iter_mut().zip([clipped, on_srgb, unproofed]) {
            *slot = slot.max(v);
        }
    }
    let [export, held, oracle_p99, oracle_max, cube_p99, cube_max, on_p3] = worst;
    let [export_twin, oracle_twin, cube_twin] = least;
    let [clipped, on_srgb, unproofed] = reached;
    assert!(export <= 2.0 && export_twin > 1000.0, "{export} {export_twin}");
    assert!(held < 1e-4 && clipped > 0.1, "kept {held}, clipped share {clipped}");
    assert!(oracle_p99 < 1e-3 && oracle_max < 2e-3 && oracle_twin > 0.01, "{oracle_p99} {oracle_max} {oracle_twin}");
    assert!(cube_p99 < 0.01 && cube_max < 0.03 && cube_twin > 0.02, "{cube_p99} {cube_max} {cube_twin}");
    assert!(on_p3 == 0.0 && on_srgb > 0.02 && unproofed > 0.005, "{on_p3} {on_srgb} {unproofed}");
}
