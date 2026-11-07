//! The picture's histogram is counted and traced for the plots on screen alone: the Scanner's screen
//! stands in the readout's place, and Pro's face back traces the readout from a fresh count at once.
//! The readout names what it draws in a legend under it, the print curve's lines on the Colour tab.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::fields::Field;
use oni_app::{Act, Bar, Editors, Lab, PanelState, Plots, Store};
use slint::ComponentHandle;
use std::time::Duration;

use crate::common;
use crate::panel_bindings::opened;

/// How far a legend under the plot stands above the tab bar's labels: the bar's inset, never the plot.
const UNDER_PLOT: f32 = 40.0;
/// The readout's box, `PlotStyle.main`.
const BOX: f32 = 150.0;

pub(crate) fn the_readout_names_its_curves_under_the_histogram() {
    let Some((rig, shell, root)) = opened("readout-legend") else { return };
    let w = &shell.window;
    let panel = w.global::<PanelState>();
    let pick = |tab: i32| {
        panel.set_tab(tab);
        panel.invoke_tab_picked(tab);
        rig.settle();
    };
    // The tab bar's label, above its block's title of the same name.
    let tab_label = |name: &str| common::placed(w, Role::Text, name).into_iter().min_by(|a, b| a.y.total_cmp(&b.y)).unwrap();
    pick(2);
    rig.shot("readout-legend-colour");
    let names = ["Histogram", "Print curve", "Inverse curve"].map(|n| common::placed(w, Role::Text, n));
    assert!(names.iter().all(|n| n.len() == 1), "the legend names the histogram and both lines once: {names:?}");
    let [histogram, curve, inverse] = names.map(|n| n[0]);
    assert!(histogram.level_with(&curve) && curve.level_with(&inverse), "one row: {histogram:?} {curve:?} {inverse:?}");
    assert!(histogram.x < curve.x && curve.x < inverse.x, "the histogram, the curve, then its inverse");
    let colour = tab_label("Colour");
    let above = colour.y - (curve.y + curve.height);
    assert!(above > 0.0 && above < UNDER_PLOT, "under the plot, above the tab bar: {above:.1} pt");

    // The Roll tab draws no line: the histogram alone is named, on the same row, the bar unmoved.
    pick(0);
    assert_eq!(common::placed(w, Role::Text, "Print curve").len() + common::placed(w, Role::Text, "Inverse curve").len(), 0, "no line, no name");
    let alone = common::the(w, Role::Text, "Histogram");
    assert_eq!((alone.x, alone.y), (histogram.x, histogram.y), "the legend keeps its place");
    assert_eq!(tab_label("Roll").y, colour.y, "the tab bar keeps its place");
    // Twin: a legend at the plot's top would stand a plot's height above the bar.
    assert!(colour.y - (curve.y - BOX + curve.height) >= UNDER_PLOT, "the measure sees a legend over the plot");
    let _ = std::fs::remove_dir_all(&root);
}

pub(crate) fn the_scanner_face_traces_no_readout_and_pro_back_traces_a_fresh_one() {
    let Some((rig, shell, root)) = opened("picture-plots") else { return };
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let (plots, bar, lab) = (w.global::<Plots>(), w.global::<Bar>(), w.global::<Lab>());
    let (output, saturation) = (|| plots.get_output_revision(), || plots.get_saturation_revision());
    let counts = || ctl.state.borrow().notches().histogram_count();
    rig.until("the output histogram", Duration::from_secs(60), || output() > 0);
    rig.settle();
    // A render asked and landed, then the time its count and trace take.
    let rendered = |ask: &dyn Fn()| {
        let renders = ctl.state.borrow().renders();
        ask();
        rig.until("the render", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
        rig.settle();
    };
    let saturation_open = |open: bool| {
        w.global::<Editors>().set_saturation_folded(!open);
        w.global::<PanelState>().invoke_fold_changed("Saturation Curve".into(), !open);
        rig.settle();
    };

    // Twin first: on Pro the same wait sees the readout traced from the render's count.
    let (traced, counted) = (output(), counts());
    rendered(&|| w.global::<Store>().invoke_step(Field::Exposure.index(), 5));
    assert!(output() > traced && counts() > counted, "Pro's readout traces each render's count");

    bar.invoke_act(Act::FaceScanner);
    rig.settle();
    assert!(lab.get_shown());
    let (on_entry, under, counted) = (output(), saturation(), counts());
    rendered(&|| lab.invoke_press(0, 1));
    assert_eq!((output(), saturation(), counts()), (on_entry, under, counted), "no plot over the picture on screen, none traced");

    // The saturation curve opened on the Scanner's Colour tab: the picture is counted for it alone.
    saturation_open(true);
    rig.until("the saturation curve's trace", Duration::from_secs(60), || saturation() > under);
    let counted = counts();
    rendered(&|| lab.invoke_press(0, 1));
    assert!(counts() > counted, "the curve open, each render is counted for it");
    assert_eq!(output(), on_entry, "no output trace while the Scanner's screen stands in its place");

    // Pro's face back traces its readout from the count of its first render.
    saturation_open(false);
    let (counted, renders) = (counts(), ctl.state.borrow().renders());
    bar.invoke_act(Act::FacePro);
    rig.until("Pro's render", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    rig.until("a fresh readout", Duration::from_secs(5), || output() > on_entry && counts() > counted);
    let _ = std::fs::remove_dir_all(&root);
}
