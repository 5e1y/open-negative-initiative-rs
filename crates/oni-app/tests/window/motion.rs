//! The interface's motion on the headless window, its clock stepped by hand: the photo flies from its
//! tile into the canvas and back, the column's photos rise under its rows, a sheet sinks away as it
//! closes; each step's shot lands in the shots folder, and Reduce Motion lays every screen at once.

use oni_app::{Act, AppSettings, Flight, Library, Motion};
use slint::ComponentHandle;
use std::time::Duration;

use crate::common;

/// The steps a transition is shot at, in ms from its start, past the travel's whole pace.
const STEPS: [u64; 6] = [0, 60, 120, 220, 360, 700];

pub(crate) fn the_photo_flies_between_its_tile_and_the_canvas_and_reduce_motion_lays_it_at_once() {
    let root = common::Root::new("motion");
    let folder = root.scans();
    for (name, tone) in [("A.tif", 0.8), ("B.tif", 0.6), ("C.tif", 0.5), ("D.tif", 0.4), ("E.tif", 0.3), ("F.tif", 0.7)] {
        common::graded_tiff(&folder.join(name), 600, 400, tone);
    }
    let Some(app) = common::launch(root).show() else { return };
    let common::App { rig, shell, .. } = &app;
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let (lib, flight) = (w.global::<Library>(), w.global::<Flight>());
    app.read(&folder);
    rig.until("the thumbnails", Duration::from_secs(60), || {
        rig.pump();
        lib.get_tiles().iter().all(|t| t.picture.size().width > 0)
    });
    rig.settle();
    rig.shot("motion-gallery");
    let shoot = |name: &str| {
        let mut at = 0;
        for step in STEPS {
            rig.advance(Duration::from_millis(step - at));
            at = step;
            rig.shot(&format!("motion-{name}-{step:03}"));
        }
    };
    let open = |i: i32| {
        std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
        lib.invoke_tile_clicked(i, false, false);
        lib.invoke_tile_clicked(i, false, false);
    };

    open(1);
    assert!(flight.get_flying() && flight.get_into_editor(), "the double-clicked tile lifts off the grid");
    assert_eq!(flight.get_tile(), 1);
    shoot("into-editor");
    assert!(!flight.get_flying(), "landed once its pace has run");
    rig.until("B rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    rig.settle();
    rig.shot("motion-editor");

    ctl.act(Act::ToggleMode);
    assert!(flight.get_flying() && !flight.get_into_editor(), "the canvas's photo heads back to its tile");
    shoot("into-gallery");
    assert_eq!(flight.get_tile(), 1, "it lands in its own tile");
    assert!(!flight.get_flying());

    // A sheet comes in on its spring and sinks away on its way out.
    let settings = w.global::<AppSettings>();
    ctl.act(Act::Settings);
    shoot("sheet-in");
    settings.set_shown(false);
    rig.pump();
    shoot("sheet-out");

    // Twin: under Reduce Motion the editor opens with nothing in flight.
    rig.settle();
    w.global::<Motion>().set_reduced(true);
    open(2);
    assert!(!flight.get_flying(), "Reduce Motion lays the editor at once");
}
