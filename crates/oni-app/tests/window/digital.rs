//! Digital picked on the Roll tab's own Film Type: the film's blocks leave the Roll tab, the faces leave
//! the bar, and the Colour block shows the white in kelvin and tint where the print's filters stood.
//! Twin: the same frame as a negative shows them all, and back to one shows them again.

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
    assert!(shown(Role::Tab, "Scanner"), "a negative's faces");
    tab(PanelTab::Color);
    assert!(value("Temperature").ends_with("EV"), "the print's filters: {}", value("Temperature"));

    film_type("Digital");
    assert_eq!(settings().mode, ConversionMode::Digital);
    for title in FILM_BLOCKS {
        assert!(!shown(Role::Text, title), "no {title} on a digital photo");
    }
    assert!(shown(Role::Text, "Film Type") && shown(Role::Text, "Scan Profile"), "its film type and a plain file's profile stay");
    assert!(!shown(Role::Tab, "Scanner") && !shown(Role::Tab, "Pro"), "no faces for a digital photo");
    println!("shot {}", rig.shot("digital-roll").display());

    // A TIFF names no As Shot: its white rests on its own D65.
    tab(PanelTab::Color);
    assert!(!shown(Role::Button, "AUTO WHITE BALANCE"), "no balance key in Digital");
    assert_eq!((value("Temperature"), value("Tint")), ("6504 K".to_string(), "Magenta +10".to_string()));
    slider("Temperature").invoke_accessible_increment_action();
    rig.settle();
    let white = settings().white.expect("the slider wrote a white");
    assert!((white.kelvin - 6505.0).abs() < 0.5 && white.tint == settings().white.unwrap().tint, "{white:?}");
    assert_eq!(value("Temperature"), "6505 K", "the slider shown is the white's own");
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
