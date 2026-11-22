//! The Lens Corrections block of a digital photo through the window's pointer: each slider dragged or
//! pressed lays its correction, writes the sidecar and moves the canvas; a stroke painted on a
//! distorted picture is kept on the scan under the hand. A film mode shows no lens.

use i_slint_backend_testing::AccessibleRole;
use oni_app::{Canvas, Library, PanelState, PanelTab, Store};
use oni_core::{Archive, EditorStore};
use oni_settings::cg::Point;
use oni_settings::lens::LensCorrection;
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::{ConversionMode, PipelineSettings};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;
use crate::film_effects::{opened, photo, press_rail};

/// A drag along the rail of the slider named `label`, from `from` to `to` of its travel.
fn drag_rail(rig: &common::Rig, w: &oni_app::MainWindow, label: &str, from: f32, to: f32) {
    let slider = common::rail(w, label);
    let at = |share: f32| LogicalPosition::new(slider.x + 10.0 + share * (slider.width - 20.0), slider.y + slider.height - 13.0);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: at(from) });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position: at(from), button: PointerEventButton::Left });
    for i in 1..=8 {
        rig.window.dispatch_event(WindowEvent::PointerMoved { position: at(from + (to - from) * i as f32 / 8.0) });
        rig.pump();
    }
    rig.window.dispatch_event(WindowEvent::PointerReleased { position: at(to), button: PointerEventButton::Left });
    rig.pump();
}

/// The share of the travel a value of −100..100 stands at.
fn share(v: f32) -> f32 {
    (v + 100.0) / 200.0
}

pub(crate) fn each_lens_slider_lays_its_correction_and_moves_the_canvas() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the Lens Corrections block is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-lens-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let name = photo(&root.join("Scans"));
    let (rig, shell) = opened(&gpu, &root);
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let panel = w.global::<PanelState>();
    panel.set_tab(PanelTab::Roll);
    panel.invoke_tab_picked(PanelTab::Roll);
    rig.settle();
    let lens = || ctl.state.borrow().store().settings().lens_correction;
    let g = w.global::<Store>();
    assert!(!common::placed(w, AccessibleRole::Text, "Lens Corrections").is_empty(), "a digital photo lays Lens Corrections");
    assert!(lens() == LensCorrection::default() && !g.get_lens_corrections_modified(), "the photo opens uncorrected");
    let before = rig.drawn().1;

    // Distortion by a drag, from its rest to a barrel lens's correction.
    let renders = ctl.state.borrow().renders();
    drag_rail(&rig, w, "Distortion", share(0.0), share(60.0));
    rig.until("the distortion rendered", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    rig.settle();
    // A drag moves by the track's own gain from where the hand caught the knob, within a few units.
    assert!((lens().distortion - 60.0).abs() < 8.0, "the drag lays Distortion near where it let go: {:?}", lens());
    assert!(g.get_lens_corrections_modified(), "the block's Reset shows");
    assert_ne!(rig.drawn().1, before, "the canvas moved");
    let side = sidecar::url_for(&root.join("Scans").join(&name));
    rig.until("the sidecar written", Duration::from_secs(30), || {
        rig.pump();
        std::fs::read_to_string(&side).ok().and_then(|t| oni_settings::document::decode(t.as_bytes()).ok()).is_some_and(|d| d.settings.lens_correction == lens())
    });
    // Twin: the drag reached Distortion alone.
    assert_eq!((lens().red, lens().blue, lens().vignetting), (0.0, 0.0, 0.0));

    // Each other slider by a press on its rail, the canvas rendered again each time.
    for (label, at, read) in [
        ("Red fringe", 90.0, (|l: LensCorrection| l.red) as fn(LensCorrection) -> f32),
        ("Blue fringe", -90.0, |l| l.blue),
        ("Vignetting", 50.0, |l| l.vignetting),
    ] {
        let renders = ctl.state.borrow().renders();
        press_rail(&rig, w, label, share(at));
        rig.until(label, Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
        rig.settle();
        assert!((read(lens()) - at).abs() < 8.0, "{label} at {}: {:?}", at, lens());
    }
    println!("shot {}", rig.shot("lens-fit").display());

    // Strong corrections both ways, shot to be judged: a pincushion lens's, then a barrel lens's.
    for (shot, distortion) in [("lens-pincushion", 0.0), ("lens-barrel", 0.98)] {
        let renders = ctl.state.borrow().renders();
        press_rail(&rig, w, "Distortion", distortion);
        rig.until(shot, Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
        rig.settle();
        println!("shot {}", rig.shot(shot).display());
    }
    // Twin: the same frame as a negative keeps its lens unshown, with no Lens Corrections block.
    let laid = lens();
    panel.invoke_film_type_picked(0);
    rig.settle();
    assert_eq!(lens(), laid, "a film mode keeps the lens for a later Digital");
    assert!(common::placed(w, AccessibleRole::Text, "Lens Corrections").is_empty(), "a negative lays no Lens Corrections");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}

const SCAN: (f64, f64) = (480.0, 320.0);

/// The first marker's centre on the canvas, in points.
fn marker(c: &Canvas) -> Option<(f32, f32)> {
    let m = c.get_marks().row_data(0)?;
    Some((m.x + m.width / 2.0, m.y + m.height / 2.0))
}

pub(crate) fn a_stroke_on_a_distorted_picture_is_kept_under_the_hand() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: strokes under a lens correction are not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-dedust-lens-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    common::plain_tiff(&folder.join("A.tif"), SCAN.0 as usize, SCAN.1 as usize, 0.5);
    let mut settings = PipelineSettings::rest(ConversionMode::Digital, false);
    settings.lens_correction.distortion = -60.0;
    sidecar::write(&settings, &folder.join("A.tif"), WriteOptions::default()).unwrap();
    // SAFETY: set before any thread of this process reads it; the patches land in the check's own folder.
    unsafe { std::env::set_var("ONI_CACHES", &caches) };
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = oni_app::app::Options { support, caches, picker: Box::new(|| None), ..Default::default() };
    let shell = oni_app::app::Shell::new(store, oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let (lib, c) = (w.global::<Library>(), w.global::<Canvas>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("A on the canvas", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1 && c.get_frame().size().width > 0);
    assert_eq!(ctl.state.borrow().store().settings().lens_correction.distortion, -60.0);
    ctl.act(oni_app::Act::Correction);
    rig.until("the De-dust picture", Duration::from_secs(30), || c.get_picture_width() > 0.0);

    // A click near a corner, where the correction moves the scan the most.
    let (fx, fy, fw, fh) = (c.get_picture_x(), c.get_picture_y(), c.get_picture_width(), c.get_picture_height());
    let clicked = (fx + fw * 0.85, fy + fh * 0.2);
    c.invoke_paint_start(clicked.0, clicked.1);
    c.invoke_paint_end();
    let s = ctl.state.borrow().store().settings().clone();
    let dot = s.corrections.last().cloned().expect("a dot");
    let wanted = s.placing(SCAN).on_scan(Point::new(0.85, 0.2));
    assert!((dot.stroke[0].x - wanted.x).abs() < 1e-6 && (dot.stroke[0].y - wanted.y).abs() < 1e-6, "{:?} for {wanted:?}", dot.stroke[0]);
    // Twin: the turns' map alone, the lens forgotten, keeps another place on the scan.
    let stale = s.geometry.placing(SCAN).on_scan(Point::new(0.85, 0.2));
    assert!(((dot.stroke[0].x - stale.x) * SCAN.0).hypot((dot.stroke[0].y - stale.y) * SCAN.1) > 3.0, "{stale:?}");
    rig.until("the dot's marker", Duration::from_secs(60), || {
        rig.pump();
        marker(&c).is_some()
    });
    let at = marker(&c).unwrap();
    assert!((at.0 - clicked.0).abs() < 1.0 && (at.1 - clicked.1).abs() < 1.0, "the marker at {at:?}, the click at {clicked:?}");
    println!("shot {}", rig.shot("dedust-lens").display());
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
