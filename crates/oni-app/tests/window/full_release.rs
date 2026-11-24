//! Every way out of the editor lets it go, its full resolution first: a build landing after the return
//! is dropped, never kept resident or rendered on the hidden canvas.

use oni_app::PanelTab;
use oni_app::app::Shell;
use oni_app::fields::Field;
use oni_app::{App, Library, PanelState, Store};
use slint::ComponentHandle;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crate::common;

pub(crate) fn a_full_resolution_landing_in_the_gallery_is_let_go() {
    let root = common::Root::new("full-release");
    let folder = root.scans();
    // Past the preview's side, so the full resolution is a build of its own.
    common::graded_tiff(&folder.join("A.tif"), 3000, 2000, 0.6);
    let Some(app) = common::launch(root).show() else { return };
    let common::App { rig, shell, .. } = &app;
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    app.edit_first();
    assert!(!ctl.state.borrow().full_built(), "the preview alone, before the setting asks the full resolution");

    // The whole-decode setting asks the full resolution at the next render; the gallery comes back before it lands.
    oni_app::render::FULL_DECODE.store(true, Ordering::Relaxed);
    let building = || ctl.state.borrow().load_bar().is_some_and(|b| b.title == "Loading full resolution");
    w.global::<Store>().invoke_step(Field::Exposure.index(), 1);
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

/// One frame, `A.tif`, past the preview's side, open in the editor and rendered.
fn opened(label: &str) -> Option<common::App> {
    let root = common::Root::new(label);
    common::graded_tiff(&root.scans().join("A.tif"), 3000, 2000, 0.6);
    let app = common::launch(root).show()?;
    let ctl = &app.shell.ctl;
    app.read(&app.root.scans());
    edit_a(&app.shell);
    app.rig.until("A rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    Some(app)
}

/// A double-clicked in the gallery.
fn edit_a(shell: &Shell) {
    let lib = shell.window.global::<Library>();
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    assert!(!shell.window.global::<App>().get_gallery(), "A in the editor");
}

fn loading_full(shell: &Shell) -> bool {
    shell.ctl.state.borrow().load_bar().is_some_and(|b| b.title == "Loading full resolution")
}

pub(crate) fn a_full_resolution_build_survives_a_quick_return() {
    let Some(app) = opened("full-return") else { return };
    let common::App { rig, shell, .. } = &app;
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    oni_app::render::FULL_DECODE.store(true, Ordering::Relaxed);
    w.global::<Store>().invoke_step(Field::Exposure.index(), 1);
    rig.until("the full resolution asked", Duration::from_secs(10), || loading_full(shell));
    let asked = oni_app::decode_gate::FULL.asked();
    // Out and back before the build lands, which only a pump lets reach the interface thread.
    ctl.act(oni_app::Act::ToggleMode);
    assert!(w.global::<App>().get_gallery());
    assert!(loading_full(shell), "the bar says the decode still running");
    ctl.act(oni_app::Act::ToggleMode);
    assert!(!w.global::<App>().get_gallery(), "the editor again");
    // A notch at once asks a render, hence the full resolution, while the first build still runs.
    w.global::<Store>().invoke_step(Field::Exposure.index(), 1);
    rig.until("A's full resolution", Duration::from_secs(60), || ctl.state.borrow().full_built());
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        rig.pump();
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(oni_app::decode_gate::FULL.asked(), asked, "the return waits on the build under way, never decodes the frame again");
    assert!(!loading_full(shell), "twin: the bar goes with the build's landing");
    oni_app::render::FULL_DECODE.store(false, Ordering::Relaxed);
}

pub(crate) fn every_way_out_lets_the_editor_go() {
    let Some(app) = opened("ways-out") else { return };
    let common::App { rig, shell, root, .. } = &app;
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    oni_app::render::FULL_DECODE.store(true, Ordering::Relaxed);
    // The full resolution, Compare and the crop: what the editor holds and must let go.
    let held = || {
        let s = ctl.state.borrow();
        (s.full_resident(), s.split().is_some(), w.global::<PanelState>().get_tab() == PanelTab::Crop)
    };
    let hold = || {
        w.global::<Store>().invoke_step(Field::Exposure.index(), 1);
        rig.until("A's full resolution", Duration::from_secs(60), || ctl.state.borrow().full_built());
        ctl.act(oni_app::Act::Compare);
        ctl.act(oni_app::Act::Framing);
        rig.pump();
        assert_eq!(held(), (true, true, true), "twin: the editor holds them before it is left");
    };
    hold();

    // The ONI Trash's row, clicked beside the photo.
    w.global::<Library>().invoke_trash_row_clicked(0);
    rig.pump();
    assert!(w.global::<App>().get_gallery(), "the trash lays in the gallery");
    assert_eq!(held(), (false, false, false), "the trash's row leaves the editor whole");

    // The open frame, alone in its roll, to the ONI Trash from the editor.
    shell.open_folder(&root.join("Scans"));
    rig.until("the folder read again", Duration::from_secs(30), || ctl.state.borrow().read_done());
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    edit_a(shell);
    hold();
    ctl.act(oni_app::Act::Remove);
    rig.landed(&ctl);
    assert!(w.global::<App>().get_gallery(), "the last frame of its roll trashed, the gallery");
    assert_eq!(held(), (false, false, false), "a trashed frame leaves the editor whole");
    oni_app::render::FULL_DECODE.store(false, Ordering::Relaxed);
}
