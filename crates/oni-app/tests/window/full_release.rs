//! The editor's full resolution let go in the gallery: a build landing after the return is dropped,
//! never kept resident or rendered on the hidden canvas.

use oni_app::app::{Options, Shell};
use oni_app::fields::Field;
use oni_app::{App, Library, Store as StoreGlobal};
use oni_core::{Archive, EditorStore};
use slint::ComponentHandle;
use slint::platform::WindowEvent;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crate::common;

pub(crate) fn a_full_resolution_landing_in_the_gallery_is_let_go() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the full resolution's release is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-full-release-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    // Past the preview's side, so the full resolution is a build of its own.
    common::graded_tiff(&folder.join("A.tif"), 3000, 2000, 0.6);
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    let lib = w.global::<Library>();
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("A rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    assert!(!ctl.state.borrow().full_built(), "the preview alone, before the setting asks the full resolution");

    // The whole-decode setting asks the full resolution at the next render; the gallery comes back before it lands.
    oni_app::render::FULL_DECODE.store(true, Ordering::Relaxed);
    let building = || ctl.state.borrow().load_bar().is_some_and(|b| b.title == "Loading full resolution");
    w.global::<StoreGlobal>().invoke_step(Field::Exposure.index(), 1);
    rig.until("the full resolution asked", Duration::from_secs(10), building);
    let rendered = ctl.state.borrow().renders();
    ctl.act(oni_app::Act::ToggleMode);
    assert!(w.global::<App>().get_gallery(), "back in the gallery");
    // Its landing, posted to the interface thread, runs among these pumps.
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(4) {
        rig.pump();
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!ctl.state.borrow().full_built(), "the full resolution is kept resident in the gallery");
    // The notch asked before the return may land; nothing is asked after it.
    assert!(ctl.state.borrow().renders() <= rendered + 1, "the hidden canvas rendered in the gallery");

    // Twin: back in the editor the same build lands and is kept.
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    ctl.act(oni_app::Act::ToggleMode);
    assert!(!w.global::<App>().get_gallery(), "the editor again");
    rig.until("A's full resolution", Duration::from_secs(60), || ctl.state.borrow().full_built());
    oni_app::render::FULL_DECODE.store(false, Ordering::Relaxed);
}
