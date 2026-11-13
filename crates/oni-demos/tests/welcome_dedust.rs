//! The welcome's De-dust scene on a synthetic dusty negative: DETECT rings what the reading found in view, REMOVE
//! marks and heals each under the number of its tile; twins: the whole frame, an idle hand, a clean frame.

use std::path::Path;

use oni_demos::stage::SCALE;
use oni_demos::photo::Rgba;
use oni_demos::welcome::dedust::{self, Ask, Scan, Shot, View};
use oni_settings::levels::Levels;
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::{ConversionMode, PipelineSettings};

/// Four specks a few pixels wide, apart enough for their numbers, and two far off any detail holding them,
/// in the frame's pixels.
const SPECKS: [(usize, usize); 4] = [(780, 420), (820, 500), (780, 580), (820, 660)];
const FAR: [(usize, usize); 2] = [(150, 150), (1400, 1000)];
const SIDE: usize = 6;

/// A negative of a grainy grey, past its window only where a speck lies, with its sidecar.
fn negative(path: &Path, dusty: bool) {
    oni_fixtures::tiff::pixels_tiff(path, 1600, 1200, |x, y| {
        let speck = dusty && SPECKS.iter().chain(&FAR).any(|&(sx, sy)| (sx..sx + SIDE).contains(&x) && (sy..sy + SIDE).contains(&y));
        let v = if speck { 0.0 } else { 0.8 + 0.1 * ((x as f32 * 0.13).sin() * (y as f32 * 0.17).cos()) };
        [v, v, v]
    });
    let mut settings = PipelineSettings::rest(ConversionMode::Negative, false);
    let window = Levels::with_ends(-0.5, 1.0);
    (settings.levels.red, settings.levels.green, settings.levels.blue) = (window, window, window);
    sidecar::write(&settings, path, WriteOptions::default()).unwrap();
}

/// A point of the strip on the frame, in pixels.
fn on_frame(shot: &Shot, p: (f32, f32)) -> (f32, f32) {
    let k = SCALE / shot.view.zoom;
    (shot.view.x as f32 + p.0 * k, shot.view.y as f32 + p.1 * k)
}

/// The frame's pixel at a point of the strip, in its film pixels.
fn pixel(shot: &Shot, frame: usize, p: (f32, f32)) -> [i32; 3] {
    let f = &shot.frames[frame];
    let (x, y) = ((shot.region.x as f32 + p.0 * SCALE) as u32, (shot.region.y as f32 + p.1 * SCALE) as u32);
    let at = ((y * f.width + x) * 4) as usize;
    [0, 1, 2].map(|c| i32::from(f.pixels[at + c]))
}

/// A speck's middle in the strip's points.
fn in_strip(shot: &Shot, (sx, sy): (usize, usize)) -> (f32, f32) {
    let k = shot.view.zoom / SCALE;
    let middle = |s: usize, from: u32| (s as f32 + SIDE as f32 / 2.0 - from as f32) * k;
    (middle(sx, shot.view.x), middle(sy, shot.view.y))
}

/// Whether the pane and the strip show the same dust: on every ringed frame the count is the rings the strip
/// holds, and at the end the tiles' numbers are the marks' numbers the strip holds.
fn agrees(shot: &Shot) -> bool {
    let (w, h) = (shot.region.width as f32 / SCALE, shot.region.height as f32 / SCALE);
    let inside = |p: &(f32, f32)| p.0 > 0.0 && p.1 > 0.0 && p.0 < w && p.1 < h;
    let counted = shot.seen.iter().filter(|s| !s.rings.is_empty()).all(|s| s.found == s.rings.iter().filter(|r| inside(r)).count() as i32);
    let last = shot.seen.last().unwrap();
    let mut numbers: Vec<i32> = last.marks.iter().filter(|m| inside(&m.1)).map(|m| m.0).collect();
    numbers.sort();
    counted && numbers == last.rows
}

#[test]
fn detect_rings_the_readings_specks_and_remove_heals_each_under_its_number() {
    if oni_app::render::Device::open().is_err() {
        return eprintln!("no GPU: the De-dust scene is not filmed");
    }
    let dir = std::env::temp_dir().join(format!("oni-welcome-dedust-check-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (dusty, clean) = (dir.join("Dusty.tif"), dir.join("Clean.tif"));
    negative(&dusty, true);
    negative(&clean, false);

    let shot = dedust::shoot(&dusty, Ask::default()).unwrap();
    assert!(shot.detected && shot.removed, "the hand's clicks reached both keys");
    let ringed: Vec<_> = shot.seen.iter().filter(|s| !s.rings.is_empty()).collect();
    assert!(!ringed.is_empty(), "rings show after DETECT");
    assert!(agrees(&shot), "the count is the strip's rings, each tile's number a mark in the strip");
    for seen in &ringed {
        assert_eq!((seen.rings.len(), seen.found), (SPECKS.len(), SPECKS.len() as i32), "the specks in view, none far off");
        for &ring in &seen.rings {
            let (x, y) = on_frame(&shot, ring);
            let on = SPECKS.iter().any(|&(sx, sy)| (x - (sx as f32 + 3.0)).abs() <= 2.0 && (y - (sy as f32 + 3.0)).abs() <= 2.0);
            assert!(on, "the ring at ({x}, {y}) lies on a speck laid");
        }
    }
    let last = shot.seen.last().unwrap();
    assert_eq!((last.rows.len(), last.pending, last.found), (SPECKS.len(), 0, 0), "every patch landed, the reading gone");
    // The pane ends under its tiles' last whole row, inside the stage; the idle twin's shows whole.
    let foot = last.foot.expect("the pane ends under its tiles");
    assert!(foot > 0.0 && foot <= dedust::STAGE.1, "{foot}");
    // Each speck stood out from the grey around it, and no longer does.
    let end = shot.frames.len() - 1;
    for &speck in &SPECKS {
        let at = in_strip(&shot, speck);
        let beside = (at.0 - 12.0, at.1);
        let off = |frame: usize| (0..3).map(|c| (pixel(&shot, frame, at)[c] - pixel(&shot, frame, beside)[c]).abs()).max().unwrap();
        assert!(off(0) > 40 && off(end) < off(0) / 3, "the speck at {speck:?} healed: {} then {}", off(0), off(end));
    }

    // Twin: the whole frame's dust ringed and removed: the count and the tiles outrun the strip.
    let whole = dedust::shoot(&dusty, Ask { whole: true, ..Ask::default() }).unwrap();
    assert!(!agrees(&whole), "a frame read whole is caught");

    // Twin: the same frame and hand, never pressing, shows no ring, no count and no tile.
    let idle = dedust::shoot(&dusty, Ask { still_hand: true, ..Ask::default() }).unwrap();
    assert!(!idle.detected && idle.seen.iter().all(|s| s.rings.is_empty() && s.found == 0 && s.rows.is_empty() && s.foot.is_none()));
    assert_eq!(idle.frames[0].pixels, idle.frames.last().unwrap().pixels, "nothing changed");

    // Twin: a clean frame read finds nothing, and REMOVE stays out of the hand's reach.
    let none = dedust::shoot(&clean, Ask { rehearsal: true, ..Ask::default() }).unwrap();
    assert!(none.detected && !none.removed, "DETECT pressed, REMOVE out of reach");
    assert!(none.seen.iter().all(|s| s.rings.is_empty() && s.found == 0 && s.rows.is_empty()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_export_is_shown_as_it_is_and_healed_on_its_own_values() {
    if oni_app::render::Device::open().is_err() {
        return eprintln!("no GPU: the export's path is not exercised");
    }
    let dir = std::env::temp_dir().join(format!("oni-welcome-dedust-export-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("Export.png");
    let (w, h) = (640u32, 480u32);
    let pixels = (0..w * h)
        .flat_map(|i| {
            let (x, y) = (i % w, i / w);
            let speck = (300..304).contains(&x) && (200..204).contains(&y);
            if speck { [8, 8, 12, 255] } else { [(120 + x / 8) as u8, (140 + y / 8) as u8, 200, 255] }
        })
        .collect();
    let export = Rgba { width: w, height: h, pixels };
    export.png(&file).unwrap();
    let scan = Scan::open(&file).unwrap();
    assert_eq!(scan.settings.mode, oni_settings::ConversionMode::Positive, "an export with no edit reads as the positive it is");
    let view = View { x: 200, y: 100, zoom: 1.0, width: 200, height: 200 };
    let shown = scan.detail(&scan.settings, view).unwrap();
    let at = |p: &Rgba, x: u32, y: u32| p.pixels[((y * p.width + x) * 4) as usize..][..4].to_vec();
    assert_eq!(at(&shown, 10, 20), at(&export, 210, 120), "the export's own values on screen");
    assert_eq!(at(&shown, 101, 101), at(&export, 301, 201), "its speck too");
    assert!(scan.as_is_drift(view).unwrap() <= 1, "the healed path gives back the export's values");
    // Twin: a view a column off is another picture, so the comparison sees a stray.
    let off = View { x: 201, ..view };
    assert_ne!(scan.detail(&scan.settings, off).unwrap().pixels, shown.pixels);
    let _ = std::fs::remove_dir_all(&dir);
}
