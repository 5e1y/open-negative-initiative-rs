//! Select Film Stock on the headless window: the default's note naming the mode in the app's language,
//! the stocks by brand with their artwork, a click picking the roll's stock, Done and Escape changing nothing,
//! a roll on a line shown with its stock unstated until a precise one is picked.

use oni_app::app::{Options, Shell};
use oni_app::{AppSettings, Library, Stocks};
use oni_core::{Archive, EditorStore};
use oni_settings::Film;
use oni_settings::film::FILM_STOCKS;
use slint::platform::WindowEvent;
use slint::{ComponentHandle, Model};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

pub(crate) fn a_rolls_stock_is_picked_by_brand_and_shown_with_its_artwork() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the stock picker is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-stocks-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let folder = root.join("Scans");
    for (name, tone) in [("A.tif", 0.8), ("B.tif", 0.5)] {
        common::linear_tiff(&folder.join(name), 96, 64, tone);
    }
    oni_library::folders::set_roll(Some(&Film::new("Roll 9")), &oni_library::paths::canonical(&folder)).unwrap();
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(store, oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    let (lib, stocks) = (w.global::<Library>(), w.global::<Stocks>());
    lib.invoke_tile_clicked(0, false, false);
    rig.pump();
    // None picked: the first photo's mode's default, said so.
    assert_eq!(lib.get_stock().as_str(), "Generic Colour Negative");
    assert!(lib.get_stock_note().starts_with("Default, from this roll's"), "{}", lib.get_stock_note());
    // In French the mode is named as its tabs name it, never in English inside a French sentence.
    let settings = w.global::<AppSettings>();
    settings.invoke_language_picked(2);
    rig.pump();
    assert_eq!(lib.get_stock_note().as_str(), "Par défaut, d’après le mode Négatif de cette pellicule");
    // Twin: the mode's English name inside the French sentence.
    assert!(!lib.get_stock_note().contains("Negative"));
    settings.invoke_language_picked(1);
    rig.pump();
    assert_eq!(lib.get_stock_note().as_str(), "Default, from this roll's Negative mode", "English again");

    // The sheet: the brands in the catalogue's order, nothing marked.
    lib.invoke_select_roll_type();
    rig.pump();
    assert!(stocks.get_shown());
    let brands = stocks.get_brands();
    let names: Vec<String> = (0..brands.row_count()).filter_map(|i| brands.row_data(i)).map(|b| b.brand.to_string()).collect();
    assert_eq!(&names[..3], ["Generic", "AgfaPhoto", "CineStill"]);
    let cells = every_cell(&stocks);
    // Every stock offered: no Foma 401, no line its stocks replace.
    assert_eq!(cells.len(), oni_settings::film::offered_stocks().count());
    assert!(!cells.iter().any(|c| c.name == "Foma Fomapan 401"), "Foma 401 is not offered");
    assert!(cells.iter().any(|c| c.name == "Foma Fomapan 400"), "twin: Foma 400 still is");
    for name in ["Kodak Portra 160", "Kodak Portra 400", "Kodak Portra 800", "Kodak Gold 200", "Kodak Vision3 500T", "Fujifilm C200", "Fujifilm Superia X-TRA 400"] {
        assert!(cells.iter().any(|c| c.name == name), "{name} listed");
    }
    assert!(!cells.iter().any(|c| c.name == "Kodak Portra" || c.name == "Fujifilm Fujicolor"), "a line its stocks replace is not offered");
    assert!(cells.iter().all(|c| c.has_artwork && !c.picked && c.note.is_empty()));
    rig.settle();
    println!("shot {}", rig.shot("stock-picker").display());

    // Escape and Done close it unchanged.
    rig.key("\u{1b}");
    assert!(!stocks.get_shown());
    lib.invoke_select_roll_type();
    stocks.invoke_chosen(-1);
    rig.pump();
    assert!(lib.get_stock_note().starts_with("Default"), "twin: Done picks nothing");

    // A roll on the Portra line, as one set before the split: its name and canister, its stock said unstated.
    let line = FILM_STOCKS.iter().position(|k| k.id == "kodak-portra").unwrap();
    lib.invoke_select_roll_type();
    stocks.invoke_chosen(line as i32);
    rig.pump();
    assert!(!stocks.get_shown());
    assert_eq!((lib.get_stock().as_str(), lib.get_stock_note().as_str()), ("Kodak Portra", "Stock not specified"));
    assert!(lib.get_stock_has_artwork());
    // Reopened, the line is shown marked among its stocks, at its catalogue place.
    lib.invoke_select_roll_type();
    rig.pump();
    let cells = every_cell(&stocks);
    assert_eq!(cells.len(), oni_settings::film::offered_stocks().count() + 1, "the roll's line shown beside the offered stocks");
    let at = cells.iter().position(|c| c.name == "Kodak Portra").expect("the roll's line is shown");
    assert!(cells[at].picked && cells[at].has_artwork && cells[at].note == "Stock not specified", "{:?}", cells[at].note);
    assert_eq!(cells[at + 1].name, "Kodak Portra 160");
    assert_eq!(cells.iter().filter(|c| c.picked).count(), 1);
    // Scrolled down to Kodak, for the shot.
    rig.window.dispatch_event(WindowEvent::PointerScrolled { position: slint::LogicalPosition::new(640.0, 500.0), delta_x: 0.0, delta_y: -1100.0 });
    rig.settle();
    println!("shot {}", rig.shot("stock-picker-line").display());

    // Portra 160 picked: the precise stock named, nothing left to say, the line gone from the sheet.
    let precise = FILM_STOCKS.iter().position(|k| k.id == "kodak-portra160").unwrap();
    stocks.invoke_chosen(precise as i32);
    rig.pump();
    assert_eq!((lib.get_stock().as_str(), lib.get_stock_note().as_str()), ("Kodak Portra 160", ""));
    lib.invoke_select_roll_type();
    rig.pump();
    let cells = every_cell(&stocks);
    let marked: Vec<String> = cells.iter().filter(|c| c.picked).map(|c| c.name.to_string()).collect();
    assert_eq!(marked, ["Kodak Portra 160"]);
    assert!(!cells.iter().any(|c| c.name == "Kodak Portra"), "twin: once a stock is picked the line is no longer shown");
    rig.key("\u{1b}");
    let _ = std::fs::remove_dir_all(&root);
}

/// Every cell the sheet shows, brand after brand.
fn every_cell(stocks: &Stocks) -> Vec<oni_app::StockCell> {
    let brands = stocks.get_brands();
    (0..brands.row_count())
        .filter_map(|i| brands.row_data(i))
        .flat_map(|b| (0..b.rows.row_count()).filter_map(move |r| b.rows.row_data(r)))
        .flat_map(|r| (0..r.cells.row_count()).filter_map(move |c| r.cells.row_data(c)))
        .collect()
}
