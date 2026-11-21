//! The grain's colour line has left Film Effects (owner's call): its rest deepens the photo's own colours where
//! the grain darkens them, and no hand moves it from the panel any more.

use i_slint_backend_testing::AccessibleRole;
use oni_settings::blocks::GrainColour;
use std::time::Duration;

use crate::common;
use crate::film_effects::{opened, photo, press_rail};

const BLACK: &str = "Grain colour at the black point";
const WHITE: &str = "Grain colour at the white point";

pub(crate) fn the_grains_colour_line_leaves_the_panel() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the grain's colour line is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-grain-colour-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    photo(&root.join("Scans"));
    let (rig, shell) = opened(&gpu, &root);
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let renders = ctl.state.borrow().renders();
    press_rail(&rig, w, "Grain", 0.3);
    rig.until("the grain rendered", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    rig.settle();
    assert_eq!(common::placed(w, AccessibleRole::Slider, "Grain size").len(), 1, "twin: the grain's own sliders stay");
    assert!(common::placed(w, AccessibleRole::Slider, BLACK).is_empty() && common::placed(w, AccessibleRole::Slider, WHITE).is_empty(), "no colour line");
    assert_eq!(ctl.state.borrow().store().settings().grain.colour, GrainColour::REST, "its rest laid");
    println!("shot {}", rig.shot("grain-colour-gone").display());
    let _ = std::fs::remove_dir_all(&root);
}
