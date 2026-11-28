//! Colour compression in the Paper section of the Colour tab: absent on no paper, laid under the paper's name on one, at
//! 100 % at rest; a number typed is taken and renders anew, held to the paper's own reach, and a rest lays it back on 1.

use crate::common;
use i_slint_backend_testing::{AccessibleRole as Role, ElementHandle};
use oni_app::fields::Field;
use oni_app::{MainWindow, PanelState, PanelTab, Store};
use oni_settings::PaperStock;
use slint::ComponentHandle;
use std::time::Duration;

const LABEL: &str = "Colour compression";

fn slider_value(w: &MainWindow) -> String {
    let mut found = ElementHandle::find_by_accessible_label(w, LABEL).filter(|e| e.accessible_role() == Some(Role::Slider));
    found.next().expect("the slider shown").accessible_value().unwrap_or_default().to_string()
}

pub(crate) fn colour_compression_lies_under_a_papers_name_and_takes_a_typed_value_within_its_reach() {
    let root = common::Root::new("colour-compression");
    common::plain_tiff(&root.scans().join("A.tif"), 960, 640, 0.35);
    let Some(app) = common::launch(root).size(1512, 1600).scale(1.0).show() else { return };
    let common::App { rig, shell, .. } = &app;
    app.edit_first();
    let (w, ctl) = (&shell.window, shell.ctl.clone());
    let (panel, store) = (w.global::<PanelState>(), w.global::<Store>());
    panel.set_tab(PanelTab::Color);
    panel.invoke_tab_picked(PanelTab::Color);
    panel.invoke_film_type_picked(0);
    rig.settle();
    let settings = || ctl.state.borrow().store().settings().clone();
    let count = || common::placed(w, Role::Slider, LABEL).len();
    let chosen = |paper: PaperStock| PaperStock::all().iter().position(|p| *p == paper).unwrap() as i32 + 1;

    // No paper, no compression.
    assert!(settings().paper().is_none() && !panel.get_compression_shown());
    assert_eq!(count(), 0);
    // Royal Digital picked: the slider under its name, above Print lift, at 100 %.
    let royal = PaperStock::of("kodak-royal-digital");
    panel.invoke_paper_chosen(chosen(royal));
    rig.settle();
    assert!(panel.get_compression_shown() && count() == 1);
    let (name, slider, lift) = (common::the(w, Role::Button, royal.name()), common::the(w, Role::Slider, LABEL), common::the(w, Role::Checkbox, "Print lift"));
    assert!(name.y < slider.y && slider.y < lift.y, "the paper, its compression, then Print lift");
    assert_eq!((settings().print.colour_compression, slider_value(w)), (1.0, "100%".to_string()));
    rig.settle();
    println!("shot {}", rig.shot("paper-block-colour-compression").display());

    // The knob stands on 100 % of Royal Digital's 0 to 120 %, its rest.
    let fraction = store.get_colour_compression_fraction();
    assert!((fraction - 1.0 / 1.2).abs() < 0.02, "{fraction}");
    // 50 typed down from 100: half the side reads, rendered anew.
    let renders = ctl.state.borrow().renders();
    store.invoke_typed(Field::ColourCompression.index(), -50);
    rig.pump();
    assert_eq!((settings().print.colour_compression, slider_value(w)), (0.5, "50%".to_string()));
    rig.until("the paper compressed less", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    // Past the paper's reach the value lands on it: Royal Digital's 120 %.
    store.invoke_typed(Field::ColourCompression.index(), 300);
    rig.pump();
    assert_eq!(settings().print.colour_compression, royal.compression_limit().unwrap());
    // The rest lays it back on 1, the paper as baked.
    store.invoke_rest(Field::ColourCompression.index());
    rig.pump();
    assert_eq!((settings().print.colour_compression, slider_value(w)), (1.0, "100%".to_string()));
    // Twin: back on no paper, the slider leaves.
    panel.invoke_paper_chosen(0);
    rig.settle();
    assert_eq!(count(), 0);
}
