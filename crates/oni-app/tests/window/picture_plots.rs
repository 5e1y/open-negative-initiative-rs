//! The picture's histogram is counted and traced for the plots on screen alone: Details' loupe
//! stands in the readout's place, and leaving it traces the readout from a fresh count at once.
//! The readout is its plot alone, no word under it. The levels' and curves' inputs are read for a plot
//! on screen alone, one opened again read at once.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::fields::Field;
use oni_app::{Editors, PanelState, PanelTab, Plots, Store};
use slint::ComponentHandle;
use std::time::Duration;

use crate::common;
use crate::panel_bindings::opened;

pub(crate) fn the_readout_is_its_plot_alone() {
    let Some(app) = opened("readout-alone") else { return };
    let common::App { rig, shell, .. } = &app;
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
}

pub(crate) fn the_curves_inputs_are_read_while_their_plot_is_open_alone() {
    let Some(app) = opened("input-plots") else { return };
    let common::App { rig, shell, .. } = &app;
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let (panel, plots, editors) = (w.global::<PanelState>(), w.global::<Plots>(), w.global::<Editors>());
    panel.set_tab(PanelTab::Color);
    panel.invoke_tab_picked(PanelTab::Color);
    let curves_open = |open: bool| {
        editors.set_curves_folded(!open);
        panel.invoke_fold_changed("Curves".into(), !open);
        rig.drawn();
    };
    let renders = || ctl.state.borrow().renders();
    curves_open(true);
    rig.until("the curves' plot", Duration::from_secs(60), || plots.get_curves_revision() > 0);
    // Twin first: open, a step's render is read and its plot traced again.
    let (traced, rendered) = (plots.get_curves_revision(), renders());
    w.global::<Store>().invoke_step(Field::Exposure.index(), 5);
    rig.until("the render", Duration::from_secs(60), || renders() > rendered);
    rig.until("the curves traced again", Duration::from_secs(10), || plots.get_curves_revision() > traced);

    // Folded, a step's render reads nothing for them.
    curves_open(false);
    rig.settle();
    let (traced, rendered) = (plots.get_curves_revision(), renders());
    w.global::<Store>().invoke_step(Field::Exposure.index(), 5);
    rig.until("the render", Duration::from_secs(60), || renders() > rendered);
    for _ in 0..20 {
        rig.settle();
    }
    assert_eq!(plots.get_curves_revision(), traced, "a folded plot's inputs are read");

    // Opened again, its inputs are read anew at once, nothing moved since.
    let rendered = renders();
    curves_open(true);
    rig.until("a render asked anew", Duration::from_secs(60), || renders() > rendered);
    rig.until("the curves traced at once", Duration::from_secs(10), || plots.get_curves_revision() > traced);
}
