//! A stroke painted on a turned, cropped picture is kept on the scan under the hand, and its marker
//! follows the picture through another turn and a mirror, the stroke itself untouched.

use oni_app::app::{Options, Shell};
use oni_app::{Canvas, Library};
use oni_core::{Archive, EditorStore};
use oni_settings::cg::{Point, Rect};
use oni_settings::frame::Geometry;
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::{ConversionMode, PipelineSettings};
use slint::{ComponentHandle, Model};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

const SCAN: (f64, f64) = (320.0, 240.0);

/// The first marker's centre on the canvas, in points.
fn marker(c: &Canvas) -> Option<(f32, f32)> {
    let m = c.get_marks().row_data(0)?;
    Some((m.x + m.width / 2.0, m.y + m.height / 2.0))
}

/// Where a scan point shows on the canvas under `g`.
fn shown(c: &Canvas, g: &Geometry, p: Point) -> (f32, f32) {
    let f = g.placing(SCAN).on_frame(p);
    (c.get_picture_x() + f.x as f32 * c.get_picture_width(), c.get_picture_y() + f.y as f32 * c.get_picture_height())
}

pub(crate) fn a_stroke_stays_on_its_scan_point_through_turns_and_mirrors() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: strokes across framings are not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-dedust-turn-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    common::plain_tiff(&folder.join("A.tif"), SCAN.0 as usize, SCAN.1 as usize, 0.5);
    let mut settings = PipelineSettings::rest(ConversionMode::Negative, false);
    settings.geometry = Geometry { quarter_turns: 1, crop: Rect::new(0.1, 0.05, 0.6, 0.8), ..Geometry::default() };
    sidecar::write(&settings, &folder.join("A.tif"), WriteOptions::default()).unwrap();
    // SAFETY: set before any thread of this process reads it; the patches land in the check's own folder.
    unsafe { std::env::set_var("ONI_CACHES", &caches) };
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(store, engine, options).unwrap();
    rig.window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, c) = (w.global::<Library>(), w.global::<Canvas>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("A on the canvas", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1 && c.get_frame().size().width > 0);
    ctl.act(oni_app::Act::Correction);
    rig.until("the De-dust picture", Duration::from_secs(30), || c.get_picture_width() > 0.0);

    // A click a quarter down the picture shown: kept where the scan holds that pixel.
    let (fx, fy, fw, fh) = (c.get_picture_x(), c.get_picture_y(), c.get_picture_width(), c.get_picture_height());
    let clicked = (fx + fw * 0.3, fy + fh * 0.25);
    c.invoke_paint_start(clicked.0, clicked.1);
    c.invoke_paint_end();
    let geometry = ctl.state.borrow().store().settings().geometry.clone();
    let dot = ctl.state.borrow().store().settings().corrections.last().cloned().expect("a dot");
    let wanted = geometry.placing(SCAN).on_scan(Point::new(0.3, 0.25));
    assert!((dot.stroke[0].x - wanted.x).abs() < 1e-6 && (dot.stroke[0].y - wanted.y).abs() < 1e-6, "{:?} for {wanted:?}", dot.stroke[0]);
    // Twin: the picture's own fraction kept as the stroke is another place on the scan.
    assert!((dot.stroke[0].x - 0.3).hypot(dot.stroke[0].y - 0.25) > 0.05, "{:?}", dot.stroke[0]);
    rig.until("the dot's marker", Duration::from_secs(60), || marker(&c).is_some());
    let at = marker(&c).unwrap();
    assert!((at.0 - clicked.0).abs() < 1.0 && (at.1 - clicked.1).abs() < 1.0, "the marker at {at:?}, the click at {clicked:?}");

    // Turned and mirrored, the stroke stays and its marker follows the scan point onto the new picture.
    for (name, act) in [("a turn", oni_app::Act::Rotate), ("a mirror", oni_app::Act::Mirror)] {
        let before = ctl.state.borrow().store().settings().geometry.clone();
        ctl.act(act);
        let g = ctl.state.borrow().store().settings().geometry.clone();
        assert_ne!(g, before, "{name} moved nothing");
        rig.until("the picture framed anew", Duration::from_secs(60), || {
            rig.pump();
            marker(&c).is_some_and(|m| {
                let e = shown(&c, &g, dot.stroke[0]);
                (m.0 - e.0).abs() < 1.0 && (m.1 - e.1).abs() < 1.0
            })
        });
        assert_eq!(ctl.state.borrow().store().settings().corrections.last().unwrap().stroke, dot.stroke, "{name} moved the stroke");
        // Twin: where the former framing put it is elsewhere on this picture.
        let stale = shown(&c, &before, dot.stroke[0]);
        let m = marker(&c).unwrap();
        assert!((m.0 - stale.0).hypot(m.1 - stale.1) > 5.0, "{name}: the marker stayed where it was");
    }
    rig.shot("dedust_turn");
    let _ = std::fs::remove_dir_all(&root);
}
