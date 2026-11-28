//! The stock's hero, its canister large over its name and its key, wherever a film is picked: Prepare the Roll's
//! Film step opens the sheet; the editor's Roll tab, which changes the roll's stock for every photo of it as one
//! undoable step, and a digital photo's Film block, which lists the virtual films alone, drop the stocks' menu.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::{Library, MainWindow, Menus, PanelState, PanelTab, Preparing, Stocks};
use oni_settings::{DyeSet, VirtualStock};
use slint::platform::{Key, WindowEvent};
use slint::{ComponentHandle, Model};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

pub(crate) fn stock(id: &str) -> i32 {
    oni_settings::film::stocks().iter().position(|k| k.id == id).unwrap() as i32
}

fn family(path: &Path) -> DyeSet {
    oni_settings::sidecar::read(path, false).unwrap().settings.dye_set
}

/// Every stock the sheet lists, by name.
fn listed(stocks: &Stocks) -> Vec<String> {
    let rows = stocks.get_rows();
    (0..rows.row_count())
        .filter_map(|r| rows.row_data(r))
        .flat_map(|r| (0..r.cells.row_count()).filter_map(move |c| r.cells.row_data(c)))
        .map(|c| c.name.to_string())
        .collect()
}

/// Whether a popup shows: the stocks' menu is the only one these checks drop.
pub(crate) fn dropped(w: &MainWindow) -> bool {
    !i_slint_core::window::WindowInner::from_pub(w.window()).active_popups().is_empty()
}

/// The menu's stocks by name, the brands' lines left out, and the brands themselves.
fn menu(stocks: &Stocks) -> (Vec<String>, Vec<String>) {
    let lines = stocks.get_lines();
    let lines: Vec<oni_app::StockLine> = (0..lines.row_count()).filter_map(|i| lines.row_data(i)).collect();
    let names = lines.iter().filter(|l| !l.brand).map(|l| l.name.to_string()).collect();
    (names, lines.iter().filter(|l| l.brand).map(|l| l.name.to_string()).collect())
}

/// The key pressed, its menu dropped under it.
pub(crate) fn drop_menu(rig: &common::Rig, w: &MainWindow) {
    common::click(rig, w, common::the(w, Role::Button, "Select Film Stock").middle());
    rig.pump();
    assert!(dropped(w), "the key drops the stocks' menu");
}

pub(crate) fn point(rig: &common::Rig, (x, y): (f32, f32)) {
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: slint::LogicalPosition::new(x, y) });
    rig.pump();
}

/// The open photo's sidecar as it reads now.
pub(crate) fn sidecar(path: &Path) -> String {
    std::fs::read_to_string(oni_settings::sidecar::url_for(path)).unwrap_or_default()
}

/// The shell on a folder of two negatives, at `height`.
fn shell_on(root: common::Root, height: u32) -> Option<(common::App, PathBuf)> {
    let folder = root.scans();
    for (name, tone) in [("A.tif", 0.8), ("B.tif", 0.5)] {
        common::graded_tiff(&folder.join(name), 240, 160, tone);
    }
    let folder = PathBuf::from(oni_library::paths::canonical(&folder));
    let app = common::launch(root).size(1280, height).show()?;
    app.read(&folder);
    Some((app, folder))
}

pub(crate) fn prepare_picks_the_stock_on_its_hero() {
    let root = common::Root::new("stock-hero-prepare");
    let Some((app, _)) = shell_on(root, 900) else { return };
    let common::App { rig, shell, .. } = &app;
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let (p, stocks) = (w.global::<Preparing>(), w.global::<Stocks>());
    ctl.act(oni_app::Act::PrepareRoll);
    rig.until("the roll's surface", Duration::from_secs(10), || p.get_shown());
    rig.settle();
    // The mode's default on the hero, its canister drawn large, its key live.
    assert_eq!(p.get_stock().name.as_str(), "Generic Colour Negative");
    assert!(p.get_stock().has_artwork);
    let key = common::the(w, Role::Button, "Select Film Stock");
    common::click(rig, w, key.middle());
    rig.pump();
    assert!(stocks.get_shown(), "the key opens the sheet");
    assert_eq!(listed(&stocks).len(), oni_settings::film::offered_stocks().len(), "every stock offered");
    stocks.invoke_chosen(stock("kodak-portra400"));
    rig.settle();
    assert!(!stocks.get_shown());
    let hero = p.get_stock();
    assert_eq!(hero.name.as_str(), "Kodak Portra 400");
    assert!(hero.has_artwork && hero.note.is_empty(), "a stock its own profile measures needs no word: {}", hero.note);
    println!("shot {}", rig.shot("stock-hero-prepare").display());
    // Twin: Done changes nothing.
    common::click(rig, w, key.middle());
    stocks.invoke_chosen(-1);
    rig.pump();
    assert_eq!(p.get_stock().name.as_str(), "Kodak Portra 400");
}

pub(crate) fn the_roll_tab_changes_the_rolls_stock_as_one_undoable_step() {
    let root = common::Root::new("stock-hero-roll");
    let Some((app, folder)) = shell_on(root, 1200) else { return };
    let common::App { rig, shell, .. } = &app;
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let (panel, stocks, lib, menus) = (w.global::<PanelState>(), w.global::<Stocks>(), w.global::<Library>(), w.global::<Menus>());
    let marker = || oni_library::folders::roll(&folder.to_string_lossy()).and_then(|f| f.stock);
    let open_dyes = || ctl.state.borrow().store().settings().dye_set;
    let shown_dyes = || ctl.state.borrow().shown_settings().map(|s| s.dye_set);
    lib.invoke_tile_opened(0);
    rig.until("the photo rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    let open = PathBuf::from(ctl.state.borrow().open_path().expect("a photo open"));
    let other = ["A.tif", "B.tif"].map(|n| folder.join(n)).into_iter().find(|p| *p != open).unwrap();
    panel.set_tab(PanelTab::Roll);
    panel.invoke_tab_picked(PanelTab::Roll);
    rig.settle();
    assert_eq!(panel.get_stock().name.as_str(), "Generic Colour Negative");
    assert!(panel.get_stock().note.starts_with("Default"), "the default said so: {}", panel.get_stock().note);
    assert_eq!(open_dyes(), DyeSet::of("generic-c41"));
    let undo_before = menus.get_live().undo;

    // The key drops the menu on every stock a roll takes, by brand, the sheet's order.
    drop_menu(rig, w);
    let (names, brands) = menu(&stocks);
    let by_brand: Vec<&str> = oni_app::stocks::by_brand(None).into_iter().flat_map(|(_, l)| l).map(|i| oni_settings::film::stocks()[i].display_name).collect();
    assert_eq!(names, by_brand, "every stock offered for a roll, by brand");
    assert_eq!(brands.first().map(String::as_str), Some("Generic"));
    assert_eq!(stocks.get_in_use(), -1, "a roll on no stock checks none, as the sheet marks none");
    println!("shot {}", rig.shot("stock-menu-roll-tab").display());

    // A stock lit shows its dyes on the photo, nothing written; the light gone, the photo's own again.
    let (renders, written) = (ctl.state.borrow().renders(), sidecar(&open));
    stocks.invoke_lit(stock("fujifilm-c200"));
    rig.until("the photo rendered on the lit stock's dyes", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    assert_eq!(shown_dyes(), Some(DyeSet::of("fujifilm-c200")), "the canvas shows the lit stock's dyes");
    assert_eq!((open_dyes(), marker()), (DyeSet::of("generic-c41"), None), "nothing written");
    assert_eq!(menus.get_live().undo, undo_before, "no step");
    stocks.invoke_lit(-1);
    rig.settle();
    assert_eq!(shown_dyes(), Some(DyeSet::of("generic-c41")), "the photo's own dyes again");
    // Twin: a stock lit that names no dyes shows the photo as it is, and asks no render.
    let renders = ctl.state.borrow().renders();
    stocks.invoke_lit(stock("ilford-hp5"));
    rig.settle();
    assert_eq!(shown_dyes(), Some(DyeSet::of("generic-c41")));
    assert_eq!(ctl.state.borrow().renders(), renders, "nothing rendered for a light that moves nothing");
    stocks.invoke_lit(-1);
    assert_eq!(common::placed(w, Role::Button, &names[0]).len(), 1, "the first line in view as the menu drops");
    // The arrows walk the long list, which scrolls each stock they light into view.
    for _ in 0..40 {
        rig.key(&slint::SharedString::from(Key::DownArrow));
    }
    let lit = ctl.state.borrow().stock_lit().expect("the arrows light a stock");
    let line = common::in_popup(w, Role::Button, oni_settings::film::stocks()[lit].display_name);
    assert!(line.y + line.height <= rig.window.size().height as f32 && line.y >= 0.0, "the lit line in view: {line:?}");
    assert!(common::placed(w, Role::Button, &names[0]).is_empty(), "twin: the list scrolled its first line out of view");
    rig.key("\u{1b}");
    rig.settle();
    assert!(!dropped(w) && ctl.state.borrow().stock_lit().is_none(), "Escape closes the menu and lights nothing");
    assert_eq!(shown_dyes(), Some(DyeSet::of("generic-c41")));
    assert_eq!(sidecar(&open), written, "the sidecar never written by a light");

    // A pick writes the roll's marker and its dyes on both photos as one step.
    drop_menu(rig, w);
    let renders = ctl.state.borrow().renders();
    stocks.invoke_picked(stock("fujifilm-c200"));
    rig.key("\u{1b}");
    rig.landed(&ctl);
    rig.until("the open photo rendered on its new dyes", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
    rig.settle();
    assert_eq!(marker().as_deref(), Some("fujifilm-c200"), "the roll's stock");
    assert_eq!((open_dyes(), family(&other)), (DyeSet::of("fujifilm-c200"), DyeSet::of("fujifilm-c200")), "every photo of the roll on its dyes");
    assert_eq!(panel.get_stock().name.as_str(), "Fujifilm C200");
    assert!(menus.get_live().undo, "one step");
    println!("shot {}", rig.shot("stock-hero-roll-tab").display());

    // One Undo takes the stock and both photos' dyes back; Redo lays them again.
    ctl.act(oni_app::Act::Undo);
    rig.landed(&ctl);
    rig.settle();
    assert_eq!((marker(), open_dyes(), family(&other)), (None, DyeSet::of("generic-c41"), DyeSet::of("generic-c41")));
    assert_eq!(panel.get_stock().name.as_str(), "Generic Colour Negative", "the hero follows the Undo");
    assert_eq!(menus.get_live().undo, undo_before);
    ctl.act(oni_app::Act::Redo);
    rig.landed(&ctl);
    rig.settle();
    assert_eq!((marker().as_deref(), open_dyes()), (Some("fujifilm-c200"), DyeSet::of("fujifilm-c200")));
    assert_eq!(panel.get_stock().name.as_str(), "Fujifilm C200", "the hero follows the Redo");
    // Twin: the stock in use picked again writes nothing, and the gallery's sidebar shows the stock the editor picked.
    drop_menu(rig, w);
    assert_eq!(menu(&stocks).0[usize::try_from(stocks.get_in_use()).unwrap() - brands_before(&stocks)], "Fujifilm C200", "the stock in use checked");
    stocks.invoke_picked(stock("fujifilm-c200"));
    rig.key("\u{1b}");
    rig.landed(&ctl);
    assert_eq!(marker().as_deref(), Some("fujifilm-c200"));
    ctl.act(oni_app::Act::ToggleMode);
    rig.settle();
    assert_eq!(lib.get_stock().name.as_str(), "Fujifilm C200");
    assert_eq!(family(&open), DyeSet::of("fujifilm-c200"), "the open photo's dyes written to its sidecar");
}

/// The brands' lines before the line in use.
fn brands_before(stocks: &Stocks) -> usize {
    let lines = stocks.get_lines();
    (0..usize::try_from(stocks.get_in_use()).unwrap()).filter(|&i| lines.row_data(i).is_some_and(|l| l.brand)).count()
}

pub(crate) fn a_digital_photos_film_is_picked_among_the_virtual_films() {
    let root = common::Root::new("stock-hero-film");
    std::fs::create_dir_all(root.join("Scans")).unwrap();
    common::graded_tiff(&root.join("Scans/A.tif"), 1200, 800, 0.7);
    let Some(app) = crate::film_effects::opened(root) else { return };
    let common::App { rig, shell, .. } = &app;
    let (ctl, w) = (shell.ctl.clone(), &shell.window);
    let (panel, stocks, menus) = (w.global::<PanelState>(), w.global::<Stocks>(), w.global::<Menus>());
    panel.set_tab(PanelTab::Color);
    panel.invoke_tab_picked(PanelTab::Color);
    rig.settle();
    let film = || ctl.state.borrow().store().settings().film().expect("a digital photo is shot on its film");
    assert_eq!(panel.get_stock().name.as_str(), VirtualStock::NEUTRAL.name());
    assert!(panel.get_stock().has_artwork, "its canister drawn");
    println!("shot {}", rig.shot("stock-hero-film").display());

    // The menu lists the virtual films alone, by brand, the one shot on checked.
    drop_menu(rig, w);
    let (names, _) = menu(&stocks);
    let by_brand: Vec<&str> = oni_app::stocks::virtual_by_brand().into_iter().flat_map(|(_, l)| l).map(|i| oni_settings::film::stocks()[i].display_name).collect();
    assert_eq!(names, by_brand);
    let offered = VirtualStock::all().into_iter().filter(|v| v.entry().offered());
    let (mut sorted, mut virtual_names) = (names.clone(), offered.map(|v| v.name().to_string()).collect::<Vec<_>>());
    sorted.sort();
    virtual_names.sort();
    assert_eq!(sorted, virtual_names, "the offered virtual films alone, every one");
    assert!(names.len() < oni_settings::film::offered_stocks().len(), "twin: a roll's menu lists more");
    assert_eq!(stocks.get_lines().row_data(usize::try_from(stocks.get_in_use()).unwrap()).unwrap().name.as_str(), VirtualStock::NEUTRAL.name(), "the film in use checked");
    assert!(stocks.get_lines().iter().filter(|l| !l.brand).all(|l| l.has_artwork), "each line with its canister");
    println!("shot {}", rig.shot("stock-menu-film").display());
    rig.key("\u{1b}");
    rig.pump();

    // Each film picked shoots the photo on it, its own light in sight, as one step an Undo takes back.
    let mut sealed = vec![];
    for &next in VirtualStock::all().iter().rev() {
        let (before, renders, undo) = (film().stock, ctl.state.borrow().renders(), menus.get_live().undo);
        drop_menu(rig, w);
        stocks.invoke_picked(stock(next.catalogue_id()));
        rig.key("\u{1b}");
        // Each step reads the store and the panel, which land with the posts: no spring of the eighty to wait out.
        rig.pump();
        assert_eq!((film().stock, panel.get_stock().name.as_str()), (next, next.name()));
        // Its light on a seal beside its name, daylight's blue or tungsten's green; the neutral film's none.
        let (balance, seal) = match next.balance() {
            oni_settings::virtual_film::Balance::Daylight => (1, Some("Daylight")),
            oni_settings::virtual_film::Balance::Tungsten => (2, Some("Tungsten")),
            oni_settings::virtual_film::Balance::Scene => (0, None),
        };
        assert_eq!(panel.get_film_balance(), balance, "{} names its light", next.name());
        assert!(seal.is_none_or(|s| !common::placed(w, i_slint_backend_testing::AccessibleRole::Text, s).is_empty()), "{}'s seal shown", next.name());
        if let Some(word) = seal.filter(|_| !sealed.contains(&balance)) {
            sealed.push(balance);
            rig.settle();
            println!("shot {}", rig.shot(&format!("film-seal-{}", word.to_lowercase())).display());
        }
        if next == before {
            assert_eq!(menus.get_live().undo, undo, "the film it stands on writes no step");
            continue;
        }
        rig.until("the photo rendered on its new film", Duration::from_secs(60), || ctl.state.borrow().renders() > renders);
        let s = ctl.state.borrow().store().settings().clone();
        assert_eq!((s.gains.stops, s.dye_set), (next.reference().stops, next.dye_set()), "placed on its own reference");
        ctl.act(oni_app::Act::Undo);
        rig.pump();
        assert_eq!(film().stock, before, "one Undo takes the film back");
        ctl.act(oni_app::Act::Redo);
        rig.pump();
    }
    // Twin: a stock no digital photo is shot on, chosen anyway, leaves the film as it is.
    let before = film();
    stocks.invoke_picked(stock("agfaphoto-apx"));
    rig.settle();
    assert_eq!(film(), before);
}
