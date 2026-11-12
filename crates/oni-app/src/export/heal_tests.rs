//! A file written heals every stroke, its patch made on the frame's whole decode when none is on disk.

use super::*;
use crate::render::{Decoded, Engine, Laid, Linear};
use oni_export::Image;
use oni_settings::cg::Point;
use oni_settings::frame::Correction;
use oni_settings::id::Id;

const W: usize = 256;
const H: usize = 192;

/// Noise smoothed over 9 pixels at a photograph's contrast, a bright speck of 5 pixels at its centre.
fn specked() -> Vec<[f32; 4]> {
    let hash = |x: usize, y: usize| {
        let mut v = (x as u32).wrapping_mul(73_856_093) ^ (y as u32).wrapping_mul(19_349_663);
        v ^= v >> 13;
        v = v.wrapping_mul(1_274_126_177);
        (v >> 8) as f32 / 16_777_216.0
    };
    let r = 4usize;
    let mut pixels: Vec<[f32; 4]> = (0..W * H)
        .map(|i| {
            let (x, y) = (i % W, i / W);
            let (mut sum, mut n) = (0.0, 0.0);
            for yy in y.saturating_sub(r)..(y + r + 1).min(H) {
                for xx in x.saturating_sub(r)..(x + r + 1).min(W) {
                    sum += hash(xx, yy);
                    n += 1.0;
                }
            }
            let v = 0.3 + 0.3 * (sum / n - 0.5);
            [v, v * 0.9, v * 0.8, 1.0]
        })
        .collect();
    for y in H / 2 - 2..=H / 2 + 2 {
        for x in W / 2 - 2..=W / 2 + 2 {
            pixels[y * W + x] = [0.95, 0.95, 0.95, 1.0];
        }
    }
    pixels
}

fn decoded() -> Decoded {
    Decoded { linear: Linear { width: W, height: H, pixels: specked() }, full_width: W as f64, undeclared: None }
}

/// The settings with one stroke on the speck, a click's two points.
fn stroked() -> PipelineSettings {
    let mut s = PipelineSettings::rest(oni_settings::ConversionMode::Positive, true);
    let id = Id::parse("0F2B6E4C-9A1D-4E7B-8C3A-5D6E7F809A1B").unwrap();
    s.corrections = vec![Correction { id, stroke: vec![Point { x: 0.5, y: 0.5 }, Point { x: 0.5 + 1e-4, y: 0.5 }], radius: 6.0 }];
    s
}

/// The green at the speck's centre over the mean of a ring 12 to 16 pixels around it.
fn speck(pixels: &[f32], w: usize, h: usize) -> f32 {
    let at = |x: usize, y: usize| pixels[(y * w + x) * 4 + 1];
    let (cx, cy) = (w / 2, h / 2);
    let around = (cy - 16..=cy + 16).flat_map(|y| (cx - 16..=cx + 16).map(move |x| (x, y)));
    let ring: Vec<f32> = around.filter(|&(x, y)| (12..=16).contains(&x.abs_diff(cx).max(y.abs_diff(cy)))).map(|(x, y)| at(x, y)).collect();
    at(cx, cy) - ring.iter().sum::<f32>() / ring.len() as f32
}

fn engine(root: &Path) -> Option<crate::render::Shared> {
    let device = crate::render::Device::open().ok()?;
    let engine = Engine::shared(&device);
    engine.lock().unwrap().caches = Some(root.join("caches"));
    Some(engine)
}

#[test]
fn an_export_heals_strokes_whose_patch_was_never_made() {
    let root = std::env::temp_dir().join(format!("oni-export-heals-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let Some(engine) = engine(&root) else {
        eprintln!("no adapter: the export's heal is not exercised");
        return;
    };
    let (s, caches) = (stroked(), root.join("caches"));
    // Twin: the render laying only what is on disk keeps the speck, and makes no patch.
    let bare = render_full(&mut engine.lock().unwrap(), decoded(), &s).unwrap();
    let kept = speck(&bare.pixels, W, H);
    assert!(kept > 0.2, "the speck shows on the bare render: {kept}");
    assert!(!crate::patches::made(&caches, &s, &s.corrections[0]));
    let healed = |what: &str| {
        let out: Image = render_full_beside(&engine, decoded(), &s).unwrap();
        let left = speck(&out.pixels, W, H);
        assert!(left.abs() < kept / 20.0, "{what}: the speck stays in the file: {left} (bare {kept})");
        assert!(crate::patches::made(&caches, &s, &s.corrections[0]), "{what}: no patch made");
    };
    healed("never made");
    // A quit purges the patches: the next export makes them again.
    crate::patches::purge(&caches);
    healed("purged at a quit");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_contact_sheet_heals_strokes_whose_patch_was_never_made() {
    let root = std::env::temp_dir().join(format!("oni-sheet-heals-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let Some(engine) = engine(&root) else {
        eprintln!("no adapter: the contact sheet's heal is not exercised");
        return;
    };
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("F.tiff");
    let data: Vec<u16> = specked().iter().flat_map(|p| [p[0], p[1], p[2]].map(|v| (v * 65535.0).round() as u16)).collect();
    tiff::encoder::TiffEncoder::new(std::fs::File::create(&path).unwrap()).unwrap().write_image::<tiff::encoder::colortype::RGB16>(W as u32, H as u32, &data).unwrap();
    let target = Target { path, settings: stroked() };
    let negative = engine.prepare(full_decode(&target.path, crate::render::Reading::of(&target.settings)).unwrap()).unwrap();
    let tile = || {
        let mut e = engine.lock().unwrap();
        let sheet = e.render_laid(&[Laid::Frame(&negative, &target.settings)], W, &|_, _| (0, 0), (W as u32, H as u32)).unwrap();
        speck(&read_laid(&mut e, sheet).unwrap().pixels, W, H)
    };
    // Twin: the sheet as it was drawn, from what is on disk alone, keeps the speck.
    let kept = tile();
    assert!(kept > 0.2, "the speck shows on the bare tile: {kept}");
    heal_ahead(&engine, &target).unwrap();
    assert!(crate::patches::made(&root.join("caches"), &target.settings, &target.settings.corrections[0]), "no patch made");
    let left = tile();
    assert!(left.abs() < kept / 20.0, "the speck stays on the sheet: {left} (bare {kept})");
    let _ = std::fs::remove_dir_all(&root);
}
