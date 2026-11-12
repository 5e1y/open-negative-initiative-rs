//! The person's collection: the bodies the rolls name taken in at launch, once; a click on the gallery's
//! empty room drops the picks and the right column shows the bodies and lenses ONI keeps; a name added there
//! lands in the support folder's file, a body typed for a roll joins it, a pick shows the roll again, an
//! export's Camera fills from it, and an empty list's key says how a name comes.

use i_slint_backend_testing::{AccessibleRole as Role, ElementHandle};
use oni_app::app::{Options, Shell, export_card};
use oni_app::{Exporting, Gear, Library};
use oni_core::{Archive, EditorStore};
use oni_library::collection::Collection;
use oni_library::index::{Entry, LibraryIndex};
use oni_settings::{Date, Film};
use slint::platform::{Key, WindowEvent};
use slint::{ComponentHandle, Model, SharedString};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

/// The names a list of the collection shows, in its order.
fn names(model: slint::ModelRc<SharedString>) -> Vec<String> {
    model.iter().map(|s| s.to_string()).collect()
}

pub(crate) fn the_collection_shows_where_nothing_is_picked_and_fills_the_exif() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the collection is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-collection-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (scans, support) = (root.join("Scans"), root.join("Support"));
    for name in ["A.tif", "B.tif"] {
        common::linear_tiff(&scans.join(name), 48, 32, 0.5);
    }
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    // A roll read before, its body typed by hand: the collection takes it in as the app starts.
    let caches = root.join("Caches");
    {
        let index = LibraryIndex::open(&LibraryIndex::default_path(&caches)).expect("the library index");
        let earlier = root.join("Earlier").join("C.tif").to_string_lossy().into_owned();
        index.upsert(&Entry::new(&earlier, Date::since_reference_date(800_000_000.0), "c-1")).unwrap();
        let mut roll = Film::new("Earlier");
        roll.camera_body = Some("Pentax MX".into());
        index.set_film(Some(&roll), &earlier).unwrap();
    }
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support: support.clone(), caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(store, oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    shell.open_folder(&scans);
    rig.until("the photos read", Duration::from_secs(30), || ctl.state.borrow().read_done() && ctl.state.borrow().order().len() == 2);
    rig.settle();
    let (lib, gear) = (w.global::<Library>(), w.global::<Gear>());
    assert_eq!(names(gear.get_bodies()), ["Pentax MX"], "the rolls' bodies taken in at launch");
    assert!(Collection::load(&support).seeded, "taken in once, kept so");
    assert_eq!(gear.get_lenses().row_count(), 0, "twin: a roll names no lens");
    let typed = |text: &str| {
        for c in text.chars() {
            rig.key(&c.to_string());
        }
        rig.key(&SharedString::from(Key::Return));
        rig.settle();
    };

    // A folder opened, nothing picked: the column stands on its roll. A click on the gallery's empty room,
    // under its last row, turns it to the collection; twin: before it, no collection.
    assert_eq!(lib.get_info_kind(), 1, "the opened roll's column");
    assert!(common::placed(w, Role::TextInput, "Add a body\u{2026}").is_empty(), "twin: no collection on a roll");
    common::click(&rig, w, (500.0, 700.0));
    rig.settle();
    assert_eq!(lib.get_info_kind(), 4, "the collection where nothing is picked");
    println!("shot {}", rig.shot("collection-empty").display());

    // A body added in its field joins the list, first, and the support folder's file.
    common::click(&rig, w, common::the(w, Role::TextInput, "Add a body\u{2026}").middle());
    typed("Leica M6");
    assert_eq!(names(gear.get_bodies()), ["Leica M6", "Pentax MX"]);
    assert_eq!(Collection::load(&support).bodies, ["Leica M6", "Pentax MX"], "kept in the support folder");
    assert_eq!(gear.get_lenses().row_count(), 0, "twin: no lens added");

    // A pick shows the roll again; the body typed for it joins the collection, first.
    lib.invoke_tile_clicked(0, false, false);
    rig.settle();
    assert_eq!(lib.get_info_kind(), 1, "a pick shows its roll again");
    common::click(&rig, w, common::the(w, Role::TextInput, "Camera Body").middle());
    typed("Nikon F3");
    rig.landed(&ctl);
    assert_eq!(names(gear.get_bodies()), ["Nikon F3", "Leica M6", "Pentax MX"], "the roll's body first in the collection");

    // An export's Camera fills from the collection: its key drops the bodies, a pick fills the line.
    ctl.act(oni_app::Act::Export);
    rig.until("the line ready", Duration::from_secs(120), || export_card::lines(w).iter().all(|l| l.ready));
    w.global::<Exporting>().set_exif_shown(true);
    rig.settle();
    let mut keys = common::placed(w, Role::Button, "Pick From Your Collection");
    keys.sort_by(|a, b| a.y.total_cmp(&b.y));
    assert_eq!(keys.len(), 2, "the Camera's key and the Lens's, no lens kept: {keys:?}");
    // No lens kept: the Lens's list says how one comes, in a line that picks nothing.
    common::click(&rig, w, keys[1].middle());
    rig.settle();
    common::click(&rig, w, common::in_popup(w, Role::Button, "No lens yet").middle());
    rig.settle();
    assert!(common::placed(w, Role::Text, "A lens typed here joins your collection").len() == 1, "its note under it");
    println!("shot {}", rig.shot("collection-none-known").display());
    rig.key(&SharedString::from(Key::Escape));
    rig.settle();
    assert!(common::placed(w, Role::Button, "No lens yet").is_empty(), "Escape closes the list");
    common::click(&rig, w, keys[0].middle());
    rig.settle();
    common::click(&rig, w, common::in_popup(w, Role::Button, "Leica M6").middle());
    rig.settle();
    let shown = |label: &str| ElementHandle::find_by_accessible_label(w, label).find_map(|e| e.accessible_value()).unwrap_or_default().to_string();
    assert_eq!(shown("Camera"), "Leica M6", "the pick fills the line's camera");
    assert_eq!(shown("Lens"), "", "twin: the lens untouched, its empty list's line picked nothing");
    assert!(common::placed(w, Role::Button, "No body yet").is_empty(), "twin: a list with names says no such line");
    println!("shot {}", rig.shot("collection-exif").display());

    // Back in the gallery, a click on its empty room shows the collection, where a lens added stays apart.
    ctl.leave_export();
    rig.settle();
    common::click(&rig, w, (500.0, 700.0));
    rig.settle();
    common::click(&rig, w, common::the(w, Role::TextInput, "Add a lens\u{2026}").middle());
    typed("Summicron 50");
    assert_eq!(names(gear.get_lenses()), ["Summicron 50"]);
    assert_eq!(gear.get_bodies().row_count(), 3, "twin: the bodies untouched");
    println!("shot {}", rig.shot("collection-filled").display());
    let _ = std::fs::remove_dir_all(&root);
}
