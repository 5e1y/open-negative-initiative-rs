//! The roll being prepared decodes first: while Prepare the Roll is shown, the gallery's thumbnails
//! and every other background decode stand still, and resume once the roll is left.

use oni_app::{Bar, Library, Preparing};
use slint::ComponentHandle;
use std::time::Duration;

use crate::common;

pub(crate) fn the_roll_prepared_decodes_while_the_gallery_stands_still() {
    let root = common::Root::new("prepare-first");
    let folder = root.join("Scans");
    let frames = 8;
    for i in 0..frames {
        common::graded_tiff(&folder.join(format!("F{i}.tif")), 600, 400, 0.2 + 0.08 * i as f32);
    }
    let Some(app) = common::launch(root).show() else { return };
    let common::App { rig, shell, .. } = &app;
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
}
