//! Digital picked on the Roll tab's own Film Type: the photo is shot on its film and its negative read as a
//! negative's, so the Roll tab keeps a negative's blocks but the roll and the copy stand's, under Source; the
//! Colour tab balances the print, with no white in kelvin, no Black and White and no picture levels. Twin: the
//! same frame as a negative shows them all under Roll, and back to one shows them again.

use i_slint_backend_testing::{AccessibleRole as Role, ElementHandle};
use oni_app::{PanelState, PanelTab};
use oni_settings::ConversionMode;
use slint::ComponentHandle;

use crate::common;

/// The negative's blocks a digital photo keeps on its film.
const NEGATIVE_BLOCKS: [&str; 3] = ["Filtration", "Highlight Detail", "Per-Channel Levels"];
/// The blocks of a scanned roll alone: its preparation and its copy stand's light.
const SCAN_BLOCKS: [&str; 2] = ["Prepared Roll", "Flat-Field"];

pub(crate) fn digital_shows_a_negatives_blocks_on_its_film() {
    let Some(app) = crate::panel_bindings::opened("digital") else { return };
    let common::App { rig, shell, .. } = &app;
    let (w, ctl) = (&shell.window, shell.ctl.clone());
    let panel = w.global::<PanelState>();
    let settings = || ctl.state.borrow().store().settings().clone();
    let tab = |t: PanelTab| {
        panel.set_tab(t);
        panel.invoke_tab_picked(t);
        rig.settle();
    };
    let shown = |role: Role, name: &str| !common::placed(w, role, name).is_empty();
    let value = |name: &str| {
        let mut found = ElementHandle::find_by_accessible_label(w, name).filter(|e| e.accessible_role() == Some(Role::Slider));
        found.next().unwrap_or_else(|| panic!("a {name} slider shown")).accessible_value().unwrap_or_default().to_string()
    };
    let film_type = |name: &str| {
        tab(PanelTab::Roll);
        common::click(rig, w, common::the(w, Role::Tab, name).middle());
        rig.settle();
    };

    // Tall enough for every block of the Roll tab to stand in view.
    rig.window.set_size(slint::PhysicalSize::new(1280, 1600));
    // Twin: a negative shows every block, its first tab its roll.
    tab(PanelTab::Roll);
    for title in NEGATIVE_BLOCKS.iter().chain(&SCAN_BLOCKS) {
        assert!(shown(Role::Text, title), "{title} on a negative");
    }
    assert!(shown(Role::Tab, "Roll") && !shown(Role::Tab, "Source"), "a negative's first tab is its roll");
    tab(PanelTab::Color);
    assert!(value("Temperature").ends_with("EV"), "the print's filters: {}", value("Temperature"));

    film_type("Digital");
    assert_eq!(settings().mode, ConversionMode::Digital);
    assert!(settings().film().is_some(), "shot on its film");
    for title in NEGATIVE_BLOCKS {
        assert!(shown(Role::Text, title), "{title} on a digital photo's negative");
    }
    for title in SCAN_BLOCKS {
        assert!(!shown(Role::Text, title), "no {title} on a digital photo");
    }
    assert!(shown(Role::Text, "Lens Corrections") && shown(Role::Tab, "Source") && !shown(Role::Tab, "Roll"), "its lens, its source");
    assert!(!shown(Role::Tab, "Scanner") && !shown(Role::Tab, "Pro"), "one face, no switch");
    println!("shot {}", rig.shot("digital-roll").display());

    tab(PanelTab::Color);
    assert!(shown(Role::Button, "AUTO WHITE BALANCE") && shown(Role::Button, "Show Auto White Balance Strength"), "a negative's balance");
    assert!(value("Temperature").ends_with("EV"), "the print's filters, never a kelvin: {}", value("Temperature"));
    assert!(shown(Role::Text, "Film") && !shown(Role::Text, "Black and White") && !shown(Role::Text, "Levels"), "its film, no switch, no picture levels");
    println!("shot {}", rig.shot("digital-colour").display());

    // Back to a negative: its blocks return.
    film_type("Negative");
    assert_eq!(settings().mode, ConversionMode::Negative);
    for title in NEGATIVE_BLOCKS.iter().chain(&SCAN_BLOCKS) {
        assert!(shown(Role::Text, title), "{title} back on the negative");
    }
}

/// The gallery's info sidebar on a folder of digital photos: its name, date and camera body, no film
/// stock nor preparation. Twin: the same folder as negatives shows its stock and asks to be prepared.
pub(crate) fn a_digital_folders_sidebar_names_no_film_stock() {
    use oni_app::{InfoKind, Library};
    let root = common::Root::new("digital-info");
    for (name, mode) in [("Phone/A.tif", ConversionMode::Digital), ("Phone/B.tif", ConversionMode::Digital), ("Negs/C.tif", ConversionMode::Negative)] {
        common::linear_tiff(&root.join(name), 48, 32, 0.5);
        let s = oni_settings::PipelineSettings::rest(mode, false);
        oni_settings::sidecar::write(&s, &root.join(name), oni_settings::sidecar::WriteOptions::default()).unwrap();
    }
    let root = root.canonical();
    let Some(app) = common::launch(root).show() else { return };
    let common::App { rig, shell, root, .. } = &app;
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
}
