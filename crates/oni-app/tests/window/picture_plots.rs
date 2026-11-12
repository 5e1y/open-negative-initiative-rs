//! The picture's histogram is counted and traced for the plots on screen alone: the Scanner's screen
//! stands in the readout's place, and Pro's face back traces the readout from a fresh count at once.
//! The readout is its plot alone, no word under it.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::fields::Field;
use oni_app::{Act, Bar, Editors, Lab, PanelState, PanelTab, Plots, Store};
use slint::ComponentHandle;
use std::time::Duration;

use crate::common;
use crate::panel_bindings::opened;

pub(crate) fn the_readout_is_its_plot_alone() {
    let Some((rig, shell, root)) = opened("readout-alone") else { return };
    let w = &shell.window;
    let (panel, plots) = (w.global::<PanelState>(), w.global::<Plots>());
    for tab in [PanelTab::Color, PanelTab::Roll] {
        panel.set_tab(tab);
        panel.invoke_tab_picked(tab);
        rig.settle();
        let named: usize = ["Histogram", "Print curve", "Inverse curve"].iter().map(|n| common::placed(w, Role::Text, n).len()).sum();
        assert_eq!(named, 0, "no word under the plot on tab {tab:?}");
        // The plot takes the readout's whole height, `PlotStyle.main`.
        let height = plots.get_output_height();
        assert!((height - 150.0).abs() < 0.5, "the plot fills the readout: {height} pt");
        // Twin: a row of words under it would leave the plot a caption and a gap shorter.
        assert!((height - (150.0 - 20.0)).abs() > 0.5);
    }
    rig.shot("readout-alone-colour");
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
