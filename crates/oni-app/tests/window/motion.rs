//! The interface's motion on the headless window, its clock stepped by hand: the photo flies from its
//! tile into the canvas and back, the column's photos rise under its rows, a sheet sinks away as it
//! closes; each step's shot lands in the shots folder, and Reduce Motion lays every screen at once.

use oni_app::{Act, AppSettings, Flight, FlightRoute, Library, Motion};
use slint::{ComponentHandle, Model};
use std::time::Duration;

use crate::common;

/// The steps a transition is shot at, in ms from its start, past the travel's whole pace.
const STEPS: [u64; 6] = [0, 80, 160, 260, 420, 1000];

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
        rig.freeze();
        let mut at = 0;
        for step in STEPS {
            rig.advance(Duration::from_millis(step - at));
            at = step;
            rig.shot(&format!("motion-{name}-{step:03}"));
        }
        rig.thaw();
    };
    // The interval waited out, the clock brought to now, so the journey's springs start from it.
    let open = |i: i32| {
        std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
        rig.freeze();
        lib.invoke_tile_clicked(i, false, false);
        lib.invoke_tile_clicked(i, false, false);
    };

    open(1);
    assert!(flight.get_flying() && flight.get_route() == FlightRoute::IntoEditor, "the double-clicked tile lifts off the grid");
    assert_eq!(flight.get_tile(), 1);
    shoot("into-editor");
    assert!(!flight.get_flying(), "landed once its pace has run");
    rig.until("B rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    rig.settle();
    rig.shot("motion-editor");

    // The export opens out of the editor, the photo moving onto its preview, and closes back onto the canvas.
    rig.freeze();
    ctl.act(Act::Export);
    assert!(flight.get_flying() && flight.get_route() == FlightRoute::IntoExport, "the canvas's photo moves onto the export's preview");
    shoot("into-export");
    rig.until("the preview", Duration::from_secs(120), || !flight.get_flying());
    rig.settle();
    rig.shot("motion-export");
    rig.freeze();
    ctl.leave_export();
    assert!(flight.get_flying() && flight.get_route() == FlightRoute::OutOfExport, "the preview moves back onto the canvas");
    shoot("out-of-export");
    rig.settle();

    rig.freeze();
    ctl.act(Act::ToggleMode);
    assert!(flight.get_flying() && flight.get_route() == FlightRoute::IntoGallery, "the canvas's photo heads back to its tile");
    shoot("into-gallery");
    assert_eq!(flight.get_tile(), 1, "it lands in its own tile");
    assert!(!flight.get_flying());

    // A sheet comes in on its spring and sinks away on its way out.
    let settings = w.global::<AppSettings>();
    rig.freeze();
    ctl.act(Act::Settings);
    shoot("sheet-in");
    rig.freeze();
    settings.set_shown(false);
    rig.pump();
    shoot("sheet-out");

    // Twin: under Reduce Motion the editor opens with nothing in flight.
    rig.settle();
    w.global::<Motion>().set_reduced(true);
    open(2);
    assert!(!flight.get_flying(), "Reduce Motion lays the editor at once");
}
