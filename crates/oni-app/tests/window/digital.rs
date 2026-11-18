//! Digital picked on the Roll tab's own Film Type: the film's blocks leave the Roll tab, the faces leave
//! the bar, and the Colour block shows the white in kelvin and tint where the print's filters stood.
//! The first tab is named Source. Twin: the same frame as a negative shows them all under Roll, and back
//! to one shows them again.

use i_slint_backend_testing::{AccessibleRole as Role, ElementHandle};
use oni_app::{PanelState, PanelTab};
use oni_settings::ConversionMode;
use slint::ComponentHandle;

use crate::common;

/// The film's blocks on the Roll tab, the frame's prepared roll among them.
const FILM_BLOCKS: [&str; 6] = ["Prepared Roll", "Flat-Field", "Filtration", "Highlight Detail", "Per-Channel Levels", "Dye Separation"];

pub(crate) fn digital_shows_its_white_and_none_of_the_films_blocks() {
    let Some((rig, shell, root)) = crate::panel_bindings::opened("digital") else { return };
    let (w, ctl) = (&shell.window, shell.ctl.clone());
    let panel = w.global::<PanelState>();
    let settings = || ctl.state.borrow().store().settings().clone();
    let tab = |t: PanelTab| {
        panel.set_tab(t);
        panel.invoke_tab_picked(t);
        rig.settle();
    };
    let shown = |role: Role, name: &str| !common::placed(w, role, name).is_empty();
    let slider = |name: &str| {
        let mut found = ElementHandle::find_by_accessible_label(w, name).filter(|e| e.accessible_role() == Some(Role::Slider));
        let one = found.next().unwrap_or_else(|| panic!("a {name} slider shown"));
        assert!(found.next().is_none(), "one {name} slider");
        one
    };
    let value = |name: &str| slider(name).accessible_value().unwrap_or_default().to_string();
    let film_type = |name: &str| {
        tab(PanelTab::Roll);
        common::click(&rig, w, common::the(w, Role::Tab, name).middle());
        rig.settle();
    };

    // Tall enough for every block of the Roll tab to stand in view.
    rig.window.set_size(slint::PhysicalSize::new(1280, 1600));
    // Twin: a negative shows its film's blocks, its faces and the print's filters in stops.
    tab(PanelTab::Roll);
    for title in FILM_BLOCKS {
        assert!(shown(Role::Text, title), "{title} on a negative");
    }
    assert!(shown(Role::Tab, "Roll") && !shown(Role::Tab, "Source"), "a negative's first tab is its roll");
    assert!(shown(Role::Tab, "Scanner"), "a negative's faces");
    tab(PanelTab::Color);
    assert!(value("Temperature").ends_with("EV"), "the print's filters: {}", value("Temperature"));

    film_type("Digital");
    assert_eq!(settings().mode, ConversionMode::Digital);
    for title in FILM_BLOCKS {
        assert!(!shown(Role::Text, title), "no {title} on a digital photo");
    }
    assert!(shown(Role::Text, "Film Type") && shown(Role::Text, "Scan Profile"), "its film type and a plain file's profile stay");
    assert!(shown(Role::Tab, "Source") && !shown(Role::Tab, "Roll"), "a digital photo's first tab is its source");
    assert!(!shown(Role::Tab, "Scanner") && !shown(Role::Tab, "Pro"), "no faces for a digital photo");
    println!("shot {}", rig.shot("digital-roll").display());

    // A TIFF names no As Shot: its white rests on its own D65.
    tab(PanelTab::Color);
    assert!(!shown(Role::Button, "AUTO WHITE BALANCE"), "no balance key in Digital");
    assert_eq!((value("Temperature"), value("Tint")), ("6504 K".to_string(), "Magenta +10".to_string()));
    slider("Temperature").invoke_accessible_increment_action();
    rig.settle();
    // An arrow's notch is even in mired: from D65 to the next round figure, 6600 K.
    let white = settings().white.expect("the slider wrote a white");
    assert!((white.kelvin - 6600.0).abs() < 0.5 && white.tint == settings().white.unwrap().tint, "{white:?}");
    assert_eq!(value("Temperature"), "6600 K", "the slider shown is the white's own");
    println!("shot {}", rig.shot("digital-colour").display());

    // Back to a negative: its blocks, faces and filters return, the white kept for a later Digital.
    film_type("Negative");
    assert_eq!((settings().mode, settings().white), (ConversionMode::Negative, Some(white)));
    for title in FILM_BLOCKS {
        assert!(shown(Role::Text, title), "{title} back on the negative");
    }
    tab(PanelTab::Color);
    assert!(value("Temperature").ends_with("EV"));
    let _ = std::fs::remove_dir_all(&root);
}

/// The gallery's info sidebar on a folder of digital photos: its name, date and camera body, no film
/// stock nor preparation. Twin: the same folder as negatives shows its stock and asks to be prepared.
pub(crate) fn a_digital_folders_sidebar_names_no_film_stock() {
    use oni_app::{InfoKind, Library};
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the info sidebar is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-digital-info-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for (name, mode) in [("Phone/A.tif", ConversionMode::Digital), ("Phone/B.tif", ConversionMode::Digital), ("Negs/C.tif", ConversionMode::Negative)] {
        common::linear_tiff(&root.join(name), 48, 32, 0.5);
        let s = oni_settings::PipelineSettings::rest(mode, false);
        oni_settings::sidecar::write(&s, &root.join(name), oni_settings::sidecar::WriteOptions::default()).unwrap();
    }
    let root = std::path::PathBuf::from(oni_library::paths::canonical(&root));
    let shots = std::env::var_os("ONI_SHOTS").map(std::path::PathBuf::from).unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    let store = oni_core::EditorStore::new(oni_core::Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = oni_app::app::Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), ..Default::default() };
    let shell = oni_app::app::Shell::new(store, oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let (w, ctl) = (&shell.window, shell.ctl.clone());
    let lib = w.global::<Library>();
    let shown = |name: &str| !common::placed(w, Role::Text, name).is_empty();
    let roll = |folder: &std::path::Path, photos: usize| {
        shell.open_folder(folder);
        rig.until("the photos read", std::time::Duration::from_secs(30), || ctl.state.borrow().read_done() && ctl.state.borrow().order().len() == photos);
        rig.settle();
        assert_eq!(lib.get_info_kind(), InfoKind::Roll, "{}", lib.get_info_empty());
    };

    roll(&root.join("Phone"), 2);
    assert!(!lib.get_roll_film() && !shown("Film Stock") && !shown("No film stock set"), "no film stock on a digital folder");
    assert!(!lib.get_roll_unmeasured() && !shown("Not prepared yet"), "nothing to prepare");
    assert!(shown("Shot Date") && shown("Camera Body"), "its date and camera body stay");
    println!("shot {}", rig.shot("digital-info").display());

    // Twin: a folder of negatives names its stock, its mode's default, and asks to be prepared.
    roll(&root.join("Negs"), 1);
    assert!(lib.get_roll_film() && shown("Film Stock") && shown("Camera Body"), "a negative's roll shows its stock");
    rig.until("the negatives told unmeasured", std::time::Duration::from_secs(10), || {
        rig.pump();
        lib.get_roll_unmeasured()
    });
    let _ = std::fs::remove_dir_all(&root);
}
