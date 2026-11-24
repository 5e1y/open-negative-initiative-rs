//! What a render reads beyond its settings reaches what the screen keeps of it: the histogram after a
//! contact sheet is the open frame's again, and a new proof draws the Details loupe again.

use oni_app::PanelTab;
use oni_app::{Canvas, Exporting, Library, PanelState};
use slint::ComponentHandle;
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

/// Three graded frames in a scratch folder, opened in a headless editor; none without an adapter.
fn opened(name: &str, side: (usize, usize)) -> Option<common::App> {
    let root = common::Root::new(name);
    let folder = root.scans();
    for (file, tone) in [("A.tif", 0.9), ("B.tif", 0.6), ("C.tif", 0.3)] {
        common::graded_tiff(&folder.join(file), side.0, side.1, tone);
    }
    let folder = PathBuf::from(oni_library::paths::canonical(&folder));
    let app = common::launch(root).show()?;
    app.read(&folder);
    app.rig.settle();
    Some(app)
}

pub(crate) fn the_histogram_after_a_sheet_is_the_open_frame_s_own() {
    let Some(app) = opened("sheet-histogram", (150, 100)) else { return };
    let common::App { rig, shell, .. } = &app;
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
}

pub(crate) fn a_new_proof_draws_the_loupe_again() {
    let Some(app) = opened("loupe-proof", (900, 600)) else { return };
    let common::App { rig, shell, .. } = &app;
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let (lib, panel) = (w.global::<Library>(), w.global::<PanelState>());
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_clicked(0, false, false);
    rig.until("A rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    panel.set_tab(PanelTab::Details);
    panel.invoke_tab_picked(PanelTab::Details);
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
}
