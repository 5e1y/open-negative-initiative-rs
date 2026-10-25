//! The base step's loupe: its first picture on a frame is the full resolution, decoded ahead from the
//! roll's entry and for the neighbours, never the reduced scan sharpening under the eye.

use oni_app::app::{Options, Shell};
use oni_app::{Bar, Library, Preparing};
use oni_core::{Archive, EditorStore};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::common;

/// Wider than the scan's 1400 pixels, so the scan and the full resolution differ.
const SIDE: (usize, usize) = (2400, 1600);

fn click(rig: &common::Rig, x: f32, y: f32) {
    let at = LogicalPosition::new(x, y);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: at });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position: at, button: PointerEventButton::Left });
    rig.window.dispatch_event(WindowEvent::PointerReleased { position: at, button: PointerEventButton::Left });
    rig.pump();
}

/// The frames: graded TIFFs, or with `ONI_RAW_DIR` three of its RAWs copied, for the real timings.
fn frames(folder: &Path) -> (Vec<String>, Option<(usize, usize)>) {
    std::fs::create_dir_all(folder).unwrap();
    if let Some(dir) = std::env::var_os("ONI_RAW_DIR") {
        let mut raws: Vec<PathBuf> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("nef") || e.eq_ignore_ascii_case("rw2"))).collect();
        raws.sort();
        let names: Vec<String> = raws.iter().take(3).map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        for (p, n) in raws.iter().zip(&names) {
            std::fs::copy(p, folder.join(n)).unwrap();
        }
        return (names, None);
    }
    let names: Vec<String> = ["A.tif", "B.tif", "C.tif"].iter().map(|n| n.to_string()).collect();
    for (n, tone) in names.iter().zip([0.9, 0.7, 0.5]) {
        common::graded_tiff(&folder.join(n), SIDE.0, SIDE.1, tone);
    }
    (names, Some(SIDE))
}

pub(crate) fn the_base_loupe_shows_the_full_resolution_first_never_the_scan() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the base loupe is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-prepare-loupe-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    let (names, size) = frames(&folder);
    let canonical = |n: &str| oni_library::paths::canonical(&folder.join(n));
    let rig = common::rig(std::env::temp_dir());
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), require_roll: true, ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, p) = (w.global::<Library>(), w.global::<Preparing>());
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(60), || ctl.state.borrow().read_done());
    rig.settle();
    let first = oni_app::app::tiles(w).into_iter().min_by_key(|t| t.index).unwrap().index;
    lib.invoke_tile_clicked(first, false, false);
    rig.pump();
    let entered = Instant::now();
    w.global::<Bar>().invoke_act(oni_app::Act::PrepareRoll);
    rig.until("the roll's surface", Duration::from_secs(30), || p.get_shown());
    let held = || ctl.state.borrow().roll_fulls();
    let (a, b, c) = (canonical(&names[0]), canonical(&names[1]), canonical(&names[2]));

    // Decoded ahead from the roll's entry: the base frame alone before its step, no neighbour yet.
    assert_eq!(p.get_step(), 0);
    rig.until("the base frame's full resolution, ahead of its step", Duration::from_secs(60), || held().contains(&a));
    println!("roll entered to the full resolution held: {:.0} ms", entered.elapsed().as_secs_f64() * 1000.0);
    assert_eq!(held(), vec![a.clone()], "before the base step only the base frame is decoded");

    // On the base step the pointer over the scan: the loupe's first picture is the full resolution.
    p.invoke_step_picked(2);
    rig.pump();
    assert_eq!(p.get_step(), 2);
    let stepped = Instant::now();
    rig.until("the scan read", Duration::from_secs(60), || p.get_scan_ready());
    let _ = rig.drawn();
    let (sx, sy) = (p.get_base_origin_x() + p.get_scan_x() + p.get_scan_width() * 0.3, p.get_base_origin_y() + p.get_scan_y() + p.get_scan_height() * 0.4);
    let hover = |dx: f32| {
        // Drawn first, so the scan's touch area, laid once the scan is read, takes the pointer.
        let _ = rig.drawn();
        rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(sx + dx, sy) });
        rig.pump();
    };
    hover(0.0);
    let scan = p.get_scan().size();
    // The rule every frame keeps: a picture in the loupe is wider than the scan, or there is none.
    let pictured = || {
        let cut = p.get_loupe_cut().row_data(2).unwrap_or(0);
        let laid = (cut > 0).then(|| p.get_loupe_source().size());
        if let Some(laid) = laid {
            assert!(laid.width > p.get_scan().size().width, "the loupe shows the scan ({laid:?})");
        }
        laid
    };
    let shown = pictured().expect("the loupe's picture at its first frame on the scan");
    println!("base step to the loupe's first picture: {:.0} ms", stepped.elapsed().as_secs_f64() * 1000.0);
    assert!(p.get_loupe_shown());
    if let Some(side) = size {
        assert_eq!((shown.width as usize, shown.height as usize), side);
        // Twin: the surface itself stays on the reduced scan, the preview path.
        assert!((scan.width as usize) < side.0, "the base surface shows the reduced scan: {scan:?}");
    }

    // The neighbour is decoded once the base frame's lands; a frame two away is not.
    rig.until("the neighbour's full resolution", Duration::from_secs(60), || held().contains(&b));
    assert!(!held().contains(&c), "twin: a frame two away is never decoded ahead");

    // A walk to a cold frame: a well with its rings, never the scan, until its own full resolution lands.
    p.invoke_base_next();
    p.invoke_base_next();
    rig.pump();
    let walked = Instant::now();
    rig.until("the cold scan read", Duration::from_secs(60), || p.get_scan_ready() && p.get_base_name().as_str() == names[2]);
    hover(1.0);
    let wells = std::cell::Cell::new(0);
    rig.until("the cold frame's full resolution in the loupe", Duration::from_secs(60), || {
        hover(0.0);
        let laid = pictured();
        if laid.is_none() {
            assert!(p.get_loupe_shown(), "the well is laid while the full resolution decodes");
            wells.set(wells.get() + 1);
        }
        laid.is_some()
    });
    println!("walk to a cold frame to its loupe's first picture: {:.0} ms, {} frames of well", walked.elapsed().as_secs_f64() * 1000.0, wells.get());
    assert!(held().contains(&c));

    // Back to the neighbour decoded ahead: its picture is there at the first frame.
    rig.until("the cold frame's neighbour", Duration::from_secs(60), || held().contains(&b));
    p.invoke_base_previous();
    rig.pump();
    let back = Instant::now();
    rig.until("the neighbour's scan read", Duration::from_secs(60), || p.get_scan_ready() && p.get_base_name().as_str() == names[1]);
    hover(2.0);
    pictured().expect("the neighbour's picture at its first frame");
    println!("walk to a neighbour to its loupe's first picture: {:.0} ms", back.elapsed().as_secs_f64() * 1000.0);
    assert!(held().len() <= 3, "at most the frame and its two neighbours: {:?}", held());

    // Past the base step, every full resolution is let go.
    let pick = (p.get_base_origin_x() + p.get_scan_x() + p.get_scan_width() * 0.05, p.get_base_origin_y() + p.get_scan_y() + p.get_scan_height() * 0.1);
    click(&rig, pick.0, pick.1);
    assert_eq!(p.get_step(), 3);
    assert!(held().is_empty(), "the flags step holds no full resolution: {:?}", held());
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
