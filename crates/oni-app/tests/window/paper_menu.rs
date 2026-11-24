//! The papers' menu under the hand on a digital photo, printed on its paper at rest: each paper it lights shown on
//! the photo with nothing written, as the stocks' menu shows a stock.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::{Menus, PanelState, PanelTab};
use oni_settings::{PaperStock, PipelineSettings};
use slint::platform::Key;
use slint::{ComponentHandle, Model};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;
use crate::stock_hero::{dropped, point, sidecar};

/// The papers' field pressed, its menu dropped under it.
fn drop_menu(rig: &common::Rig, w: &oni_app::MainWindow) {
    let title = w.global::<PanelState>().get_paper_title();
    common::click(rig, w, common::the(w, Role::Button, &title).middle());
    rig.pump();
    assert!(dropped(w), "the field drops the papers' menu");
}

/// The menu's lines as it lays them: each paper's place in the catalogue, -1 a maker's name, and its name.
fn lines(panel: &PanelState) -> Vec<(i32, String)> {
    let lines = panel.get_paper_lines();
    (0..lines.row_count()).filter_map(|i| lines.row_data(i)).map(|l| (if l.brand { -1 } else { l.index }, l.name.to_string())).collect()
}

/// A digital photo's papers under the hand: listed by maker, the one in use checked; the paper under the pointer or
/// the arrows shows on the photo with nothing written, no step and no sidecar; leaving the menu or Escape shows its
/// own paper again; a click or Return writes the paper lit as one step an Undo takes back.
pub(crate) fn a_paper_under_the_pointer_shows_on_the_photo_unwritten() {
    let root = common::Root::new("paper-menu-hover");
    std::fs::create_dir_all(root.join("Scans")).unwrap();
    common::graded_tiff(&root.join("Scans/A.tif"), 1200, 800, 0.7);
    let Some(app) = crate::film_effects::opened(root) else { return };
    let common::App { rig, shell, .. } = &app;
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let (panel, menus) = (w.global::<PanelState>(), w.global::<Menus>());
    panel.set_tab(PanelTab::Color);
    panel.invoke_tab_picked(PanelTab::Color);
    rig.settle();
    let path = PathBuf::from(ctl.state.borrow().open_path().expect("a photo open"));
    let stored = || ctl.state.borrow().store().settings().clone();
    let shown = || ctl.state.borrow().shown_settings().cloned().expect("the canvas asked");
    let on = |s: &PipelineSettings| s.paper();
    let printed = |s: &PipelineSettings, p: PaperStock| PipelineSettings { print_paper: Some(p), ..s.clone() };
    rig.advance(Duration::from_secs(5));
    rig.settle();
    let (own, undo, written) = (stored(), menus.get_live().undo, sidecar(&path));
    let own_paper = own.paper().expect("a digital photo prints on its paper at rest");

    // The menu lists every paper of the catalogue once, under its maker, the one in use checked.
    drop_menu(rig, w);
    let laid = lines(&panel);
    let order: Vec<PaperStock> = laid.iter().filter(|(i, _)| *i >= 0).map(|(i, _)| PaperStock::all()[*i as usize]).collect();
    let mut sorted = order.iter().map(|p| PaperStock::all().iter().position(|q| q == p).unwrap()).collect::<Vec<_>>();
    sorted.sort();
    assert_eq!(sorted, (0..PaperStock::all().len()).collect::<Vec<_>>(), "every paper once");
    let makers: Vec<&str> = laid.iter().filter(|(i, _)| *i < 0).map(|(_, n)| n.as_str()).collect();
    let mut maker = "";
    for (i, name) in &laid {
        if *i < 0 {
            maker = name;
        } else {
            assert!(PaperStock::all()[*i as usize].name().starts_with(maker), "{name} under {maker}");
        }
    }
    let in_use = laid.get(panel.get_paper_in_use() as usize).map(|(i, _)| *i);
    assert_eq!(in_use.map(|i| PaperStock::all()[i as usize]), Some(own_paper), "the paper in use checked");
    // Twin: a maker's line names no paper and is listed once.
    assert!(makers.iter().all(|m| PaperStock::all().iter().all(|p| p.name() != *m)));
    assert_eq!(makers.len(), makers.iter().collect::<std::collections::BTreeSet<_>>().len());
    let others: Vec<PaperStock> = order.iter().copied().filter(|p| *p != own_paper).collect();
    let (next, third) = (others[0], others[others.len() - 1]);
    let label = |p: PaperStock| p.name().to_string();

    // The pointer on a line shows its paper on the photo; nothing is written.
    let renders = ctl.state.borrow().renders();
    point(rig, common::in_popup(w, Role::Button, &label(next)).middle());
    rig.until("the photo printed on the paper under the pointer", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    rig.settle();
    assert_eq!(on(&shown()), Some(next), "the canvas shows the paper under the pointer");
    assert_eq!(shown(), printed(&own, next), "as the pick would write it");
    assert_eq!((stored(), menus.get_live().undo), (own.clone(), undo), "nothing written, no step");
    println!("shot {}", rig.shot("paper-menu-hover").display());
    // Another line, then the pointer out of the menu: the photo's own paper again, the menu still open.
    point(rig, common::in_popup(w, Role::Button, &label(third)).middle());
    rig.settle();
    assert_eq!(on(&shown()), Some(third), "the light follows the pointer");
    point(rig, (2.0, 2.0));
    rig.settle();
    assert!(dropped(w), "leaving the menu keeps it open");
    assert_eq!((on(&shown()), ctl.state.borrow().paper_lit()), (Some(own_paper), None), "the photo's own paper again");
    // Escape shows its own paper again too.
    point(rig, common::in_popup(w, Role::Button, &label(next)).middle());
    rig.settle();
    assert_eq!(on(&shown()), Some(next));
    rig.key("\u{1b}");
    rig.settle();
    assert!(!dropped(w), "Escape closes the menu");
    assert_eq!((shown(), ctl.state.borrow().paper_lit()), (own.clone(), None), "Escape shows the photo's own paper");
    // Twin: the shown settings did move while a paper was lit, so the checks above see a preview.
    assert_ne!(printed(&own, next), own);
    rig.advance(Duration::from_secs(5));
    rig.settle();
    assert_eq!((stored(), sidecar(&path), menus.get_live().undo), (own.clone(), written.clone(), undo), "no write, no sidecar, no step");

    // The arrows walk the papers from the one in use, past the makers' names, toward the longer side; Return
    // writes the one lit.
    drop_menu(rig, w);
    let at = order.iter().position(|p| *p == own_paper).unwrap();
    let (toward, back, by) = if at + 2 < order.len() { (Key::DownArrow, Key::UpArrow, 1) } else { (Key::UpArrow, Key::DownArrow, -1) };
    let lit = || ctl.state.borrow().paper_lit();
    rig.key(&slint::SharedString::from(toward));
    rig.settle();
    assert_eq!(lit(), Some(order[(at as isize + by) as usize]), "an arrow lights the paper beside the one in use");
    rig.key(&slint::SharedString::from(toward));
    rig.key(&slint::SharedString::from(back));
    rig.settle();
    let walked = order[(at as isize + by) as usize];
    assert_eq!(lit(), Some(walked), "and back");
    assert_eq!(on(&shown()), Some(walked));
    rig.key(&slint::SharedString::from(Key::Return));
    rig.settle();
    assert!(!dropped(w), "Return closes the menu");
    assert_eq!((on(&stored()), lit()), (Some(walked), None), "Return writes the paper lit");
    assert_eq!(shown(), stored(), "the photo shows what was written");
    assert!(menus.get_live().undo, "one step");
    ctl.act(oni_app::Act::Undo);
    rig.settle();
    assert_eq!(stored(), own, "one Undo takes it back");

    // A click writes the paper under the pointer, its light's render the only one; Undo takes it back.
    rig.settle();
    drop_menu(rig, w);
    let renders = ctl.state.borrow().renders();
    common::click(rig, w, common::in_popup(w, Role::Button, &label(next)).middle());
    rig.settle();
    assert_eq!(ctl.state.borrow().renders(), renders + 1, "the pick renders nothing past what its light showed");
    assert!(!dropped(w), "a click closes the menu");
    assert_eq!(stored(), printed(&own, next), "the click writes the paper under the pointer, as the light showed it");
    assert_eq!((shown(), panel.get_paper_title().to_string()), (stored(), label(next)));
    rig.advance(Duration::from_secs(5));
    rig.settle();
    assert_ne!(sidecar(&path), written, "twin: a pick writes the sidecar");
    ctl.act(oni_app::Act::Undo);
    rig.settle();
    assert_eq!((stored(), shown()), (own.clone(), own), "Undo takes the pick back");
}
