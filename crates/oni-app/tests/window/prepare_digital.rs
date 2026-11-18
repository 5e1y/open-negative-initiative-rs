//! Prepare the Roll on a digital photo: the editor's menu item opens the sheet on Digital, its film step
//! alone, the keycap PREPARE ROLL; pressed, every photo of the roll is digital with no roll reference, one
//! undo taking both back. Twin: a negative's film step asks what it was digitised with and goes on with NEXT.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::app::{Options, Shell};
use oni_app::{Act, App, Library, Menus, Preparing};
use oni_core::{Archive, EditorStore};
use oni_settings::frame::RollReference;
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::{ConversionMode, Id, PipelineSettings};
use slint::platform::WindowEvent;
use slint::{ComponentHandle, Model};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

pub(crate) fn a_digital_photos_roll_is_prepared_on_its_film_step_alone() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: Prepare on a digital photo is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-prepare-digital-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let folder = root.join("Phone");
    let canonical = |name: &str| PathBuf::from(oni_library::paths::canonical(&folder.join(name)));
    // A measured roll whose first photo was turned digital by hand: the second still a negative on it.
    let reference = RollReference { id: Id::new(), stops: [0.0; 3], black: [0.0; 3], white: [1.0; 3], base: None, median: None };
    for (name, mode) in [("A.tif", ConversionMode::Digital), ("B.tif", ConversionMode::Negative)] {
        common::graded_tiff(&folder.join(name), 240, 160, 0.5);
        let mut s = PipelineSettings::rest(mode, false);
        s.roll_reference = Some(reference.clone());
        sidecar::write(&s, &canonical(name), WriteOptions::default()).unwrap();
    }
    let (a, b) = (canonical("A.tif"), canonical("B.tif"));
    let stored = |p: &Path| sidecar::read(p, false).unwrap().settings;
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), require_roll: true, ..Options::default() };
    let shell = Shell::new(store, oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let (lib, p) = (w.global::<Library>(), w.global::<Preparing>());
    let settings = || ctl.state.borrow().store().settings().clone();
    let shown = |role: Role, name: &str| !common::placed(w, role, name).is_empty();
    let press = |role: Role, name: &str| {
        rig.settle();
        common::click(&rig, w, common::the(w, role, name).middle());
        rig.pump();
    };
    let skipped = || (0..5).map(|i| p.get_skipped().row_data(i).unwrap()).collect::<Vec<bool>>();

    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    rig.settle();
    let first = oni_app::app::tiles(w).iter().map(|t| t.index).min().unwrap();
    lib.invoke_tile_opened(first);
    rig.until("A open", Duration::from_secs(30), || ctl.state.borrow().open_path().is_some_and(|o| o.ends_with("A.tif")));
    rig.settle();
    assert_eq!(settings().mode, ConversionMode::Digital);

    // The editor's menu item, live on a digital photo, opens the sheet on Digital.
    assert!(!p.get_shown() && w.global::<Menus>().get_live().measure_roll, "Prepare the Roll… live in the editor on a digital photo");
    w.global::<App>().invoke_menu(Act::PrepareRoll);
    rig.until("the roll's surface", Duration::from_secs(30), || p.get_shown() && p.get_cells().row_count() == 2);
    rig.until("every frame read", Duration::from_secs(30), || p.get_capture() == 0 && p.get_film() == 3);
    rig.settle();
    assert!(p.get_digital() && shown(Role::Tab, "Digital"), "the film step opens on Digital");
    assert!(!shown(Role::Tab, "DSLR Scan"), "nothing to say what a digital photo was digitised with");
    assert_eq!(skipped(), [false, true, true, true, true], "a digital roll walks its film step alone");
    assert!(shown(Role::Button, "PREPARE ROLL") && !shown(Role::Button, "NEXT") && p.get_action_enabled());
    println!("shot {}", rig.shot("prepare-digital").display());

    // Twin: a negative's film step asks what it was digitised with and goes on with NEXT.
    press(Role::Tab, "Negative");
    assert!(!p.get_digital() && shown(Role::Tab, "DSLR Scan"));
    assert!(shown(Role::Button, "NEXT") && !shown(Role::Button, "PREPARE ROLL"));
    assert_eq!(skipped(), [false, true, false, false, false], "a scanner's negatives walk to their base");

    // Back on Digital, the keycap prepares the roll: both photos digital, no roll reference, one step.
    press(Role::Tab, "Digital");
    assert_eq!(p.get_step(), 0);
    press(Role::Button, "PREPARE ROLL");
    rig.until("the sheet closed", Duration::from_secs(30), || !p.get_shown());
    rig.settle();
    assert!(!w.global::<App>().get_gallery(), "the editor on the roll's first photo");
    assert_eq!(ctl.state.borrow().open_path().map(str::to_string), Some(a.to_string_lossy().into_owned()));
    assert_eq!((settings().mode, settings().roll_reference), (ConversionMode::Digital, None));
    assert_eq!((stored(&b).mode, stored(&b).roll_reference), (ConversionMode::Digital, None));
    assert!(oni_settings::prepare::read(Path::new(&oni_library::paths::canonical(&folder))).is_empty(), "no draft left in the folder");
    ctl.act(Act::Undo);
    rig.pump();
    assert_eq!(settings().roll_reference.map(|r| r.id), Some(reference.id), "one undo brings the open photo's roll back");
    assert_eq!((stored(&b).mode, stored(&b).roll_reference.map(|r| r.id)), (ConversionMode::Negative, Some(reference.id)), "and the other's");
    ctl.act(Act::Redo);
    rig.pump();
    assert_eq!(stored(&b).mode, ConversionMode::Digital);

    // A digital photo opened in the editor asks for no roll, as a slide does.
    ctl.act(Act::ToggleMode);
    rig.pump();
    rig.settle();
    lib.invoke_tile_opened(oni_app::app::tiles(w).iter().map(|t| t.index).max().unwrap());
    rig.until("B open", Duration::from_secs(30), || ctl.state.borrow().open_path().is_some_and(|o| o.ends_with("B.tif")));
    rig.settle();
    assert!(!p.get_shown(), "no Prepare the Roll on opening a digital photo");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
