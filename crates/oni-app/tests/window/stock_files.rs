//! A stock file of the person's own dropped, edited, broken and removed while the app runs: the menu lists it, the photo
//! shot on it renders again on each edit, a broken edit is told with the last version kept, a removal names the key missing.

use oni_app::{PanelState, PanelTab, Stocks, Told};
use oni_settings::VirtualStock;
use serde_json::Value;
use slint::ComponentHandle;
use slint::Model;
use std::path::Path;
use std::time::Duration;

use crate::common;

const MINE: VirtualStock = VirtualStock::of("kodak-gold-200-mine");

/// Gold 200's file as a stock of the person's own named `name`, lit at `balance`, its reference left out.
fn mine(name: &str, balance: &str) -> Vec<u8> {
    let bytes = oni_settings::stocks::shipped_files().iter().find(|(n, _)| *n == "kodak-gold200.json").map(|(_, b)| *b).unwrap();
    let mut v: Value = serde_json::from_slice(bytes).unwrap();
    v["key"] = "kodak-gold200-mine".into();
    v["name"] = name.into();
    let film = v["film"].as_object_mut().unwrap();
    film.insert("key".into(), "kodak-gold-200-mine".into());
    film.insert("dyes".into(), "kodak-gold-200".into());
    film.insert("balance".into(), balance.into());
    film.remove("aliases");
    film.remove("reference");
    serde_json::to_vec_pretty(&v).unwrap()
}

/// The names the stocks' menu lists, the brands left out.
fn listed(w: &oni_app::MainWindow) -> Vec<String> {
    let lines = w.global::<Stocks>().get_lines();
    (0..lines.row_count()).filter_map(|i| lines.row_data(i)).filter(|l| !l.brand).map(|l| l.name.to_string()).collect()
}

fn save(folder: &Path, bytes: &[u8]) {
    // Written whole beside it, then renamed in: the watch never reads half a file.
    let part = folder.join(".mine.json.part");
    std::fs::write(&part, bytes).unwrap();
    std::fs::rename(&part, folder.join("mine.json")).unwrap();
}

pub(crate) fn a_stock_file_dropped_edited_and_removed_updates_the_menu_and_the_photo() {
    let root = common::Root::new("stock-files");
    std::fs::create_dir_all(root.join("Scans")).unwrap();
    common::graded_tiff(&root.join("Scans/A.tif"), 1200, 800, 0.7);
    let Some(app) = crate::film_effects::opened(root) else { return };
    let common::App { rig, shell, root, .. } = &app;
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let folder = root.join("Support/Stocks");
    assert!(folder.is_dir(), "the support folder holds the person's stocks' folder");
    let panel = w.global::<PanelState>();
    panel.set_tab(PanelTab::Color);
    panel.invoke_tab_picked(PanelTab::Color);
    rig.settle();
    let hero = || panel.get_stock().name.to_string();
    let renders = || ctl.state.borrow().renders();
    assert!(!listed(w).iter().any(|n| n.contains("(Mine)")), "twin: no stock of the person's yet");

    // Dropped: the menu lists it, and a pick shoots the photo on it.
    save(&folder, &mine("Kodak Gold 200 (Mine)", "daylight"));
    rig.until("the menu lists the dropped stock", Duration::from_secs(30), || listed(w).iter().any(|n| n == "Kodak Gold 200 (Mine)"));
    let n = renders();
    let at = oni_settings::film::stocks().iter().position(|k| k.id == "kodak-gold200-mine").expect("listed") as i32;
    crate::stock_hero::drop_menu(rig, w);
    w.global::<Stocks>().invoke_picked(at);
    rig.key("\u{1b}");
    rig.until("the photo rendered on the person's stock", Duration::from_secs(60), || renders() > n);
    rig.settle();
    assert_eq!(hero(), "Kodak Gold 200 (Mine)");
    assert_eq!(ctl.state.borrow().store().settings().film().map(|f| f.stock), Some(MINE), "the pick shot it on the person's stock");
    let counted = || ctl.state.borrow().histogram().cloned();
    let daylight = counted();
    assert!(daylight.is_some(), "the render counted");
    println!("shot {}", rig.shot("stock-file-daylight").display());

    // Edited: renamed and lit at tungsten, the menu, the hero and the photo follow, nothing written.
    let n = renders();
    save(&folder, &mine("Kodak Gold 200 (Edited)", "tungsten"));
    rig.until("the menu lists the edited name", Duration::from_secs(30), || listed(w).iter().any(|n| n == "Kodak Gold 200 (Edited)"));
    rig.until("the photo rendered again on the edit", Duration::from_secs(60), || renders() > n);
    rig.settle();
    assert_eq!(hero(), "Kodak Gold 200 (Edited)");
    assert!(!listed(w).iter().any(|n| n == "Kodak Gold 200 (Mine)"), "the name before is gone");
    assert_eq!(ctl.state.borrow().store().settings().film().map(|f| f.stock), Some(MINE), "the photo keeps its key");
    rig.until("the edit's render counted", Duration::from_secs(30), || counted() != daylight);
    println!("shot {}", rig.shot("stock-file-tungsten").display());

    // Broken: told by its name, the last version that read stays in use.
    let told = w.global::<Told>();
    save(&folder, b"{\"oni\": \"film\", \"format\": 1,");
    rig.until("the broken file told", Duration::from_secs(30), || told.get_shown() && told.get_message().contains("mine.json"));
    assert!(told.get_message().contains("stays in use"), "{}", told.get_message());
    assert_eq!(hero(), "Kodak Gold 200 (Edited)", "the last version that read stays");

    // Removed: gone from the menu, the photo's key named missing, rendered on the default stock.
    let n = renders();
    std::fs::remove_file(folder.join("mine.json")).unwrap();
    rig.until("the menu no longer lists it", Duration::from_secs(30), || !listed(w).iter().any(|n| n.contains("Gold 200 (")));
    rig.until("the photo rendered again", Duration::from_secs(60), || renders() > n);
    rig.settle();
    assert_eq!(hero(), "kodak-gold-200-mine (file missing)");
    assert!(!MINE.is_known() && MINE.reference() == VirtualStock::default().reference());
}
