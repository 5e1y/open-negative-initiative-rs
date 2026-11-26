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

/// Forty frames of a roll never prepared, one opened from the gallery: Prepare shows the roll's sheet, its
/// cells flying in from their tiles one after the other, those the gallery laid off screen rising from the
/// sheet's foot, the photo opened ringed; the twin, Reduce Motion, lays them at once.
pub(crate) fn a_roll_opened_from_the_gallery_lands_on_its_sheet_as_a_wave() {
    let root = common::Root::new("wave");
    let folder = root.scans();
    for i in 0..40 {
        common::graded_tiff(&folder.join(format!("F{i:02}.tif")), 150, 100, 0.2 + 0.015 * i as f32);
    }
    let Some(app) = common::launch(root).with(|o| o.require_roll = true).show() else { return };
    let common::App { rig, shell, .. } = &app;
    let w = &shell.window;
    let (lib, prepare) = (w.global::<Library>(), w.global::<Preparing>());
    app.read(&folder);
    rig.until("the thumbnails", Duration::from_secs(120), || {
        rig.pump();
        slint::Model::iter(&lib.get_tiles()).filter(|t| t.picture.size().width > 0).count() >= 24
    });
    rig.settle();
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    rig.freeze();
    lib.invoke_tile_clicked(7, false, false);
    lib.invoke_tile_clicked(7, false, false);
    rig.until("Prepare the Roll", Duration::from_secs(30), || prepare.get_shown());
    assert!(!w.global::<oni_app::Flight>().get_flying(), "no flight goes on over Prepare the Roll");
    rig.until("the cells", Duration::from_secs(30), || slint::Model::row_count(&prepare.get_cells()) == 40);
    let from = prepare.get_wave_from();
    let known = slint::Model::iter(&from).filter(|p| p.width > 0.0).count();
    assert_eq!(slint::Model::row_count(&from), 40, "a place for every cell");
    assert!(known > 0 && known < 40, "the tiles laid on screen leave from there, the others from the sheet's foot: {known}");
    assert_eq!(prepare.get_opened_cell(), 7, "the photo opened is ringed");
    let mut at = 0;
    for step in [0u64, 60, 160, 300, 500, 800, 1400] {
        rig.advance(Duration::from_millis(step - at));
        at = step;
        rig.shot(&format!("wave-{step:04}"));
    }
    rig.thaw();
}
