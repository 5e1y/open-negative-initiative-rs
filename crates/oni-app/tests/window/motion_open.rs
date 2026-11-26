//! Opening a photo from the gallery on the headless window, its clock held still: the tile's picture flies
//! and the screen's own photo shows only once it has landed, in the editor and on Prepare the Roll.

use oni_app::{Flight, Library, Preparing};
use slint::ComponentHandle;
use std::time::Duration;

use crate::common;

const STEPS: [u64; 6] = [0, 80, 160, 260, 420, 1000];

fn opened(require_roll: bool, name: &str) {
    let root = common::Root::new(name);
    let folder = root.scans();
    for (file, tone) in [("A.tif", 0.8), ("B.tif", 0.6), ("C.tif", 0.5), ("D.tif", 0.4)] {
        common::graded_tiff(&folder.join(file), 600, 400, tone);
    }
    let Some(app) = common::launch(root).with(|o| o.require_roll = require_roll).show() else { return };
    let common::App { rig, shell, .. } = &app;
    let w = &shell.window;
    let (lib, flight) = (w.global::<Library>(), w.global::<Flight>());
    app.read(&folder);
    rig.until("the thumbnails", Duration::from_secs(60), || {
        rig.pump();
        slint::Model::iter(&lib.get_tiles()).all(|t| t.picture.size().width > 0)
    });
    rig.settle();
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    rig.freeze();
    lib.invoke_tile_clicked(1, false, false);
    lib.invoke_tile_clicked(1, false, false);
    assert!(flight.get_flying(), "{name}: the tile lifts off the grid");
    let mut at = 0;
    for step in STEPS {
        rig.advance(Duration::from_millis(step - at));
        at = step;
        rig.shot(&format!("{name}-{step:03}"));
        // A roll never prepared opens on Prepare as its roll is read, the clock held mid-flight.
        if require_roll && step == 0 {
            rig.until("Prepare the Roll", Duration::from_secs(30), || w.global::<Preparing>().get_shown());
            assert!(!flight.get_flying(), "{name}: a flight into the editor ends as Prepare takes the screen");
            rig.shot(&format!("{name}-prepared"));
            break;
        }
    }
    rig.thaw();
    assert!(!flight.get_flying(), "{name}: landed once its pace has run");
}

pub(crate) fn the_photo_opened_in_the_editor_shows_once_the_tile_has_landed() {
    opened(false, "open-editor");
}

pub(crate) fn the_photo_opened_on_prepare_shows_once_the_tile_has_landed() {
    opened(true, "open-prepare");
}
