//! Colour compression in the Paper section of the Colour tab: absent on no paper, laid under the paper's name on one, on
//! that paper's own dose; a number typed is taken and renders anew, held to the paper's reach, and a rest lays the dose back.

use crate::common;
use i_slint_backend_testing::{AccessibleRole as Role, ElementHandle};
use oni_app::fields::Field;
use oni_app::{MainWindow, PanelState, PanelTab, Store};
use oni_settings::PaperStock;
use slint::{ComponentHandle, Model};
use std::time::Duration;

const LABEL: &str = "Colour compression";

fn slider_value(w: &MainWindow) -> String {
    let mut found = ElementHandle::find_by_accessible_label(w, LABEL).filter(|e| e.accessible_role() == Some(Role::Slider));
    found.next().expect("the slider shown").accessible_value().unwrap_or_default().to_string()
}

pub(crate) fn colour_compression_lies_under_a_papers_name_on_its_dose_and_takes_a_typed_share_within_its_reach() {
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
    // Royal Digital picked: the slider under its name, above Print lift, on its own dose, the share of chroma it takes.
    let royal = PaperStock::of("kodak-royal-digital");
    let scale = royal.compression().expect("a scale");
    panel.invoke_paper_chosen(chosen(royal));
    rig.settle();
    assert!(panel.get_compression_shown() && count() == 1);
    let (name, slider, lift) = (common::the(w, Role::Button, royal.name()), common::the(w, Role::Slider, LABEL), common::the(w, Role::Checkbox, "Print lift"));
    assert!(name.y < slider.y && slider.y < lift.y, "the paper, its compression, then Print lift");
    let shown = |v: f32| format!("{:.0}%", 100.0 * v);
    assert_eq!((settings().print.colour_compression, slider_value(w)), (None, shown(scale.rest())));
    // The knob stands on its dose along the scale's 0 to 60 %, the same rail for every paper.
    let fraction = store.get_colour_compression_fraction();
    assert!((fraction - scale.rest() / oni_settings::tone::Print::COMPRESSION_RANGE.1).abs() < 0.02, "{fraction}");
    rig.settle();
    println!("shot {}", rig.shot("paper-block-colour-compression").display());

    // 10 typed off its dose: the hand's share, rendered anew.
    let renders = ctl.state.borrow().renders();
    store.invoke_typed(Field::ColourCompression.index(), -10);
    rig.pump();
    let less = settings().print.colour_compression.expect("a share");
    assert!((less - (scale.rest() - 0.1)).abs() < 0.006 && slider_value(w) == shown(less), "{less}");
    rig.until("the paper compressed less", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    // Past the paper's reach the share lands on it.
    store.invoke_typed(Field::ColourCompression.index(), 300);
    rig.pump();
    assert_eq!(settings().print.colour_compression, Some(scale.reach()));
    // The rest lays it back on its dose.
    store.invoke_rest(Field::ColourCompression.index());
    rig.pump();
    assert_eq!((settings().colour_compression(), slider_value(w)), (scale.rest(), shown(scale.rest())));
    // One scale for every paper: Edge's rail spans the same 0 to 60 %, veiled past its reach, where the knob stops.
    let edge = PaperStock::of("kodak-ektacolor-edge");
    let reach = edge.compression().expect("a scale").reach();
    panel.invoke_paper_chosen(chosen(edge));
    rig.settle();
    let scale = oni_settings::tone::Print::COMPRESSION_RANGE.1;
    let beyond = || store.get_beyond().row_data(Field::ColourCompression.index() as usize).unwrap_or(0.0);
    assert!((beyond() - (1.0 - reach / scale)).abs() < 1e-3, "{}", beyond());
    store.invoke_typed(Field::ColourCompression.index(), 300);
    rig.settle();
    assert_eq!(settings().print.colour_compression, Some(reach));
    assert!((store.get_colour_compression_fraction() - reach / scale).abs() < 0.02, "{}", store.get_colour_compression_fraction());
    println!("shot {}", rig.shot("paper-block-compression-edge").display());
    // Twin: on DP II the same rail lets the knob pass Edge's reach.
    let dpii = PaperStock::of("fujifilm-crystal-archive-digital-dpii");
    panel.invoke_paper_chosen(chosen(dpii));
    rig.settle();
    store.invoke_typed(Field::ColourCompression.index(), 300);
    rig.settle();
    assert!(settings().colour_compression() > reach + 0.1 && beyond() < 0.05, "{} {}", settings().colour_compression(), beyond());
    println!("shot {}", rig.shot("paper-block-compression-dpii").display());
    // Twin: back on no paper, the slider leaves.
    panel.invoke_paper_chosen(0);
    rig.settle();
    assert_eq!(count(), 0);
}
