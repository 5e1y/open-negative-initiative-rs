//! The stocks' menu under the hand on a digital photo: each stock it lights shown on the photo with nothing written.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::{Menus, PanelState, PanelTab, Stocks};
use oni_settings::VirtualStock;
use oni_settings::film::FILM_STOCKS;
use slint::platform::{Key, WindowEvent};
use slint::ComponentHandle;
use std::path::PathBuf;
use std::time::Duration;

use crate::common;
use crate::stock_hero::{dropped, drop_menu, point, sidecar};

/// A digital photo's menu under the hand: the stock under the pointer or the arrows shows on the photo with
/// nothing written, no step and no sidecar; leaving the menu or Escape shows its own film again; a click or
/// Return writes the stock lit as one step an Undo takes back.
pub(crate) fn a_stock_under_the_pointer_shows_on_the_photo_unwritten() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the stocks' menu under the hand is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-stock-menu-hover-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("Scans")).unwrap();
    common::graded_tiff(&root.join("Scans/A.tif"), 1200, 800, 0.7);
    let (rig, shell) = crate::film_effects::opened(&gpu, &root);
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let (panel, menus) = (w.global::<PanelState>(), w.global::<Menus>());
    panel.set_tab(PanelTab::Color);
    panel.invoke_tab_picked(PanelTab::Color);
    rig.settle();
    let path = PathBuf::from(ctl.state.borrow().open_path().expect("a photo open"));
    let stored = || ctl.state.borrow().store().settings().clone();
    let shown = || ctl.state.borrow().shown_settings().cloned().expect("the canvas asked");
    let on = |s: &oni_settings::PipelineSettings| s.film().map(|f| f.stock);
    // The writes the opening armed land first, so the sidecar stands still unless a pick writes it.
    rig.advance(Duration::from_secs(5));
    rig.settle();
    let (own, undo, written) = (stored(), menus.get_live().undo, sidecar(&path));
    assert_eq!(on(&own), Some(VirtualStock::KodakPortra800));
    let next = VirtualStock::ALL.iter().copied().find(|v| *v != VirtualStock::KodakPortra800).unwrap();
    let third = VirtualStock::ALL.iter().copied().rev().find(|v| *v != VirtualStock::KodakPortra800 && *v != next).unwrap();

    // The pointer on a line shows its film on the photo; nothing is written.
    drop_menu(&rig, w);
    let renders = ctl.state.borrow().renders();
    point(&rig, common::in_popup(w, Role::Button, next.name()).middle());
    rig.until("the photo rendered on the stock under the pointer", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    rig.settle();
    assert_eq!(on(&shown()), Some(next), "the canvas shows the stock under the pointer");
    assert_eq!(shown(), own.on_stock(next), "as the pick would write it");
    assert_eq!((stored(), menus.get_live().undo), (own.clone(), undo), "nothing written, no step");
    println!("shot {}", rig.shot("stock-menu-hover").display());
    // Another line, then the pointer out of the menu: the photo's own film again, the menu still open.
    point(&rig, common::in_popup(w, Role::Button, third.name()).middle());
    rig.settle();
    assert_eq!(on(&shown()), Some(third), "the light follows the pointer");
    point(&rig, (2.0, 2.0));
    rig.settle();
    assert!(dropped(w), "leaving the menu keeps it open");
    assert_eq!((on(&shown()), ctl.state.borrow().stock_lit()), (Some(VirtualStock::KodakPortra800), None), "the photo's own film again");
    // Escape shows its own film again too.
    point(&rig, common::in_popup(w, Role::Button, next.name()).middle());
    rig.settle();
    assert_eq!(on(&shown()), Some(next));
    rig.key("\u{1b}");
    rig.settle();
    assert!(!dropped(w), "Escape closes the menu");
    assert_eq!(shown(), own, "Escape shows the photo's own film");
    // Twin: the shown settings did move while a stock was lit, so the checks above see a preview.
    assert_ne!(own.on_stock(next), own);
    rig.advance(Duration::from_secs(5));
    rig.settle();
    assert_eq!((stored(), sidecar(&path), menus.get_live().undo), (own.clone(), written.clone(), undo), "no write, no sidecar, no step");

    // The arrows light the stocks after the one in use, past the brands' names; Return writes the one lit.
    drop_menu(&rig, w);
    let order: Vec<usize> = oni_app::stocks::virtual_by_brand().into_iter().flat_map(|(_, l)| l).collect();
    let at = order.iter().position(|&i| FILM_STOCKS[i].id == VirtualStock::KodakPortra800.catalogue_id()).unwrap();
    rig.key(&slint::SharedString::from(Key::DownArrow));
    rig.settle();
    let after = order.get(at + 1).copied().unwrap_or(order[at]);
    assert_eq!(ctl.state.borrow().stock_lit(), Some(after), "↓ lights the stock after the one in use");
    rig.key(&slint::SharedString::from(Key::UpArrow));
    rig.key(&slint::SharedString::from(Key::UpArrow));
    rig.settle();
    let lit = ctl.state.borrow().stock_lit().expect("↑ lights a stock");
    assert_eq!(lit, order[at.saturating_sub(1)], "↑↑ lights the stock before the one in use");
    let lit_stock = oni_app::stocks::virtual_stock(FILM_STOCKS[lit].id).unwrap();
    assert_eq!(on(&shown()), Some(lit_stock));
    rig.key(&slint::SharedString::from(Key::Return));
    rig.settle();
    assert!(!dropped(w), "Return closes the menu");
    assert_eq!((on(&stored()), ctl.state.borrow().stock_lit()), (Some(lit_stock), None), "Return writes the stock lit");
    assert_eq!(shown(), stored(), "the photo shows what was written");
    assert!(menus.get_live().undo, "one step");
    ctl.act(oni_app::Act::Undo);
    rig.settle();
    assert_eq!(stored(), own, "one Undo takes it back");

    // A click writes the stock under the pointer, its light's render the only one; Undo takes it back.
    rig.settle();
    drop_menu(&rig, w);
    let renders = ctl.state.borrow().renders();
    common::click(&rig, w, common::in_popup(w, Role::Button, next.name()).middle());
    rig.settle();
    assert_eq!(ctl.state.borrow().renders(), renders + 1, "the pick renders nothing past what its light showed");
    assert!(!dropped(w), "a click closes the menu");
    assert_eq!(on(&stored()), Some(next), "the click writes the stock under the pointer");
    assert_eq!(stored(), own.on_stock(next), "as the light showed it");
    assert_eq!(shown(), stored());
    rig.advance(Duration::from_secs(5));
    rig.settle();
    assert_ne!(sidecar(&path), written, "twin: a pick writes the sidecar");
    ctl.act(oni_app::Act::Undo);
    rig.settle();
    assert_eq!((stored(), shown()), (own.clone(), own), "Undo takes the pick back");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
