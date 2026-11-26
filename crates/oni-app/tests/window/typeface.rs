//! The app's faces seen on its main screens, at the smallest window and in each language it speaks: the
//! gallery, the editor's three panes, the export, Settings and Prepare the Roll, each shot for the eye.

use oni_app::preferences::{self, Preferences};
use oni_app::{Act, AppSettings, PanelState, PanelTab, Preparing};
use slint::ComponentHandle;
use std::time::Duration;

use crate::common;

fn screens(language: &str) {
    let root = common::Root::new(&format!("typeface-{language}"));
    let folder = root.scans();
    for (name, tone) in [("A.tif", 0.8), ("B.tif", 0.6), ("C.tif", 0.4)] {
        common::graded_tiff(&folder.join(name), 600, 400, tone);
    }
    Preferences::load(&root.support()).set(preferences::LANGUAGE, language.into()).unwrap();
    let Some(app) = common::launch(root).size(1100, 680).show() else { return };
    let common::App { rig, shell, .. } = &app;
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let shot = |screen: &str| {
        rig.settle();
        rig.shot(&format!("typeface-{language}-{screen}"));
    };
    app.read(&folder);
    shot("gallery");
    app.edit_first();
    let panel = w.global::<PanelState>();
    for (tab, name) in [(PanelTab::Color, "colour"), (PanelTab::Roll, "roll"), (PanelTab::Details, "details")] {
        panel.set_tab(tab);
        panel.invoke_tab_picked(tab);
        shot(name);
    }
    ctl.act(Act::Export);
    rig.until("the export's line", Duration::from_secs(120), || oni_app::app::export_card::card(&ctl).rows.iter().all(Option::is_some));
    shot("export");
    ctl.leave_export();
    ctl.act(Act::Settings);
    shot("settings");
    w.global::<AppSettings>().set_shown(false);
    ctl.act(Act::ToggleMode);
    rig.settle();
    ctl.act(Act::PrepareRoll);
    shot("prepare");
    assert!(w.global::<Preparing>().get_shown(), "Prepare the Roll shows");
}

pub(crate) fn the_faces_on_the_main_screens_in_english() {
    screens("en");
}

pub(crate) fn the_faces_on_the_main_screens_in_french() {
    screens("fr");
}

pub(crate) fn the_faces_on_the_main_screens_in_spanish() {
    screens("es");
}
