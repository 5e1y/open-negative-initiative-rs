//! A digital photo on the Scanner face: the bar keeps both faces, the Colour tab lays the machine, the
//! keypad and its effects where Pro's Colour and Lighting stand, and keeps Film Effects, Levels, Curves,
//! the Colour Mixer and the Saturation Curve. Twin: back on Pro, Lighting and More Color Settings return.

use i_slint_backend_testing::AccessibleRole as Role;
use i_slint_backend_testing::ElementHandle;
use oni_app::{Act, Bar, Lab, PanelState, PanelTab};
use oni_settings::{ConversionMode, Face};
use slint::ComponentHandle;
use std::time::Duration;

use crate::common;

const KEPT: [&str; 6] = ["Film Effects", "Levels", "Curves", "Colour Mixer", "Saturation Curve", "Colour"];
const SCANNER: [&str; 5] = ["Machine", "Density", "Look", "Effects", "Scanner: a minilab's density and colour, in steps"];
const PRO: [&str; 2] = ["Lighting", "More Color Settings"];

pub(crate) fn a_digital_photo_turns_to_the_scanner_face_and_keeps_its_other_blocks() {
    let Some((rig, shell, root)) = crate::panel_bindings::opened("digital-scanner") else { return };
    let (w, ctl) = (&shell.window, shell.ctl.clone());
    let (panel, bar, lab) = (w.global::<PanelState>(), w.global::<Bar>(), w.global::<Lab>());
    let settings = || ctl.state.borrow().store().settings().clone();
    let shown = |name: &str| !common::placed(w, Role::Text, name).is_empty();
    let kelvin = || ElementHandle::find_by_accessible_label(w, "Temperature").any(|e| e.accessible_value().is_some_and(|v| v.ends_with(" K")));
    // Tall enough for the whole Colour tab to stand in view.
    rig.window.set_size(slint::PhysicalSize::new(1280, 2600));
    panel.invoke_film_type_picked(3);
    rig.settle();
    assert_eq!(settings().mode, ConversionMode::Digital);
    panel.set_tab(PanelTab::Color);
    panel.invoke_tab_picked(PanelTab::Color);
    rig.settle();
    assert!(!common::placed(w, Role::Tab, "Scanner").is_empty() && !common::placed(w, Role::Tab, "Pro").is_empty(), "a digital photo's faces");
    // Twin first: Pro lays Lighting, More Color Settings and the white in kelvin, no keypad.
    for title in PRO {
        assert!(shown(title), "{title} on Pro");
    }
    assert!(kelvin() && !lab.get_shown() && !shown("Density"));

    bar.invoke_act(Act::FaceScanner);
    rig.settle();
    assert!(lab.get_shown() && bar.get_face() == 1 && settings().face == Face::Scanner, "the Scanner face turned");
    assert_eq!(lab.get_machine_line(), "ONI · DIGITAL");
    for title in SCANNER.iter().chain(&KEPT) {
        assert!(shown(title), "{title} on the Scanner face");
    }
    for title in PRO {
        assert!(!shown(title), "no {title} on the Scanner face");
    }
    assert!(!kelvin() && common::placed(w, Role::Button, "AUTO WHITE BALANCE").is_empty(), "no white in kelvin on the Scanner face");
    println!("shot {}", rig.shot("digital-scanner").display());

    // D + prints the photo lighter: the key lands in the clicks and renders.
    let renders = ctl.state.borrow().renders();
    let plus = common::the(w, Role::Button, "D +: toward light");
    common::click(&rig, w, plus.middle());
    rig.until("the keyed render", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    assert_eq!(settings().scanner.d, 1);
    assert_eq!(settings().print, oni_settings::PipelineSettings::rest(ConversionMode::Digital, false).print, "Pro's Lighting is untouched");

    bar.invoke_act(Act::FacePro);
    rig.settle();
    assert!(!lab.get_shown() && settings().face == Face::Pro);
    for title in PRO.iter().chain(&KEPT) {
        assert!(shown(title), "{title} back on Pro");
    }
    assert_eq!(settings().scanner.d, 1, "the keys wait on their face");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
