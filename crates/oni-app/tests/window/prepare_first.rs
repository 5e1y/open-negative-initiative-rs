//! The roll being prepared decodes first: while Prepare the Roll is shown, the gallery's thumbnails
//! and every other background decode stand still, and resume once the roll is left.

use oni_app::app::{Options, Shell};
use oni_app::{Bar, Library, Preparing};
use oni_core::{Archive, EditorStore};
use slint::platform::WindowEvent;
use slint::ComponentHandle;
use std::time::Duration;

use crate::common;

pub(crate) fn the_roll_prepared_decodes_while_the_gallery_stands_still() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the roll's priority over the gallery is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-prepare-first-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let folder = root.join("Scans");
    let frames = 8;
    for i in 0..frames {
        common::graded_tiff(&folder.join(format!("F{i}.tif")), 600, 400, 0.2 + 0.08 * i as f32);
    }
    let rig = common::rig(std::env::temp_dir());
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(store, oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let p = w.global::<Preparing>();

    // Every place held, so the gallery's thumbnail pass waits for one as the roll opens over it.
    let places = [oni_app::decode_gate::slot(), oni_app::decode_gate::slot()];
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(60), || ctl.state.borrow().read_done());
    let first = oni_app::app::tiles(w).into_iter().min_by_key(|t| t.index).unwrap().index;
    w.global::<Library>().invoke_tile_clicked(first, false, false);
    rig.pump();
    w.global::<Bar>().invoke_act(oni_app::Act::PrepareRoll);
    rig.until("the roll's surface", Duration::from_secs(30), || p.get_shown());
    let (taken, landed) = (oni_app::decode_gate::background_taken(), ctl.state.borrow().thumbnails_landed());
    drop(places);

    rig.until("every frame of the roll previewed", Duration::from_secs(60), || ctl.state.borrow().roll_previewed() == frames);
    rig.settle();
    assert_eq!(oni_app::decode_gate::background_taken(), taken, "no background decode took a place while the roll was prepared");
    assert_eq!(ctl.state.borrow().thumbnails_landed(), landed, "and no thumbnail of the gallery was drawn");

    // Twin: the roll left, the gallery's pass takes its places again and draws its thumbnails.
    p.invoke_cancel();
    rig.until("the roll left", Duration::from_secs(10), || !p.get_shown());
    rig.until("the gallery's decodes resumed", Duration::from_secs(60), || oni_app::decode_gate::background_taken() > taken);
    rig.until("its thumbnails drawn", Duration::from_secs(60), || ctl.state.borrow().thumbnails_landed() > landed);
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
