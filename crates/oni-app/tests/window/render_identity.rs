//! What a render reads beyond its settings reaches what the screen keeps of it: the histogram after a
//! contact sheet is the open frame's again, and a new proof draws the Details loupe again.

use oni_app::app::{Options, Shell};
use oni_app::{Canvas, Exporting, Library, PanelState};
use oni_core::{Archive, EditorStore};
use slint::ComponentHandle;
use slint::platform::WindowEvent;
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

/// Three graded frames in a scratch folder, opened in a headless editor; none without an adapter.
fn opened(name: &str, side: (usize, usize)) -> Option<(common::Rig, Shell, PathBuf, PathBuf)> {
    let gpu = oni_app::render::Device::open().map_err(|e| eprintln!("{e}: {name} is not exercised")).ok()?;
    let root = std::env::temp_dir().join(format!("oni-app-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for (file, tone) in [("A.tif", 0.9), ("B.tif", 0.6), ("C.tif", 0.3)] {
        common::graded_tiff(&folder.join(file), side.0, side.1, tone);
    }
    let folder = PathBuf::from(oni_library::paths::canonical(&folder));
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(store, engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || shell.ctl.state.borrow().read_done());
    rig.settle();
    Some((rig, shell, folder, root))
}

pub(crate) fn the_histogram_after_a_sheet_is_the_open_frame_s_own() {
    let Some((rig, shell, _, root)) = opened("sheet-histogram", (150, 100)) else { return };
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let lib = w.global::<Library>();
    let tile = |name: &str| oni_app::app::tiles(w).iter().find(|t| t.caption == name).unwrap_or_else(|| panic!("{name}")).index;
    let histogram = || ctl.state.borrow().histogram().cloned();
    lib.invoke_tile_clicked(tile("B"), false, false);
    lib.invoke_tile_opened(tile("B"));
    rig.until("B's histogram", Duration::from_secs(60), || histogram().is_some());
    rig.settle();
    let own = histogram();
    // Twin: the sheet's histogram is another one, so the same comparison sees which is shown. The
    // contact-sheet key shows the whole roll together.
    ctl.act(oni_app::Act::SelectAll);
    let status = || w.global::<Canvas>().get_status().to_string();
    rig.until("the sheet", Duration::from_secs(60), || status() == "3 frames together");
    rig.until("the sheet's histogram", Duration::from_secs(60), || histogram() != own);
    // The contact-sheet key pressed again: the open frame alone.
    ctl.act(oni_app::Act::SelectAll);
    rig.until("B alone", Duration::from_secs(60), || status().contains(" × "));
    rig.until("B's own histogram again", Duration::from_secs(60), || histogram() == own);
    let _ = std::fs::remove_dir_all(&root);
}

pub(crate) fn a_new_proof_draws_the_loupe_again() {
    let Some((rig, shell, _, root)) = opened("loupe-proof", (900, 600)) else { return };
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let (lib, panel) = (w.global::<Library>(), w.global::<PanelState>());
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("A rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    panel.set_tab(3);
    panel.invoke_tab_picked(3);
    rig.until("the loupe", Duration::from_secs(60), || {
        rig.pump();
        ctl.state.borrow().loupe_shown()
    });
    rig.settle();
    // Twin: with nothing changed, no loupe is drawn again.
    let shown = ctl.state.borrow().loupes();
    rig.settle();
    assert_eq!(ctl.state.borrow().loupes(), shown, "nothing draws the loupe on its own");
    w.global::<Exporting>().invoke_space_picked(0);
    rig.until("the loupe under the new proof", Duration::from_secs(60), || {
        rig.pump();
        ctl.state.borrow().loupes() > shown
    });
    let _ = std::fs::remove_dir_all(&root);
}
