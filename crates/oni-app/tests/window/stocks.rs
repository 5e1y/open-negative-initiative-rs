//! Select Film Stock on the headless window: the default's note naming the mode in the app's language,
//! the stocks by brand with their artwork, a click picking the roll's stock, Done and Escape changing nothing,
//! a roll on a line shown with its stock unstated until a precise one is picked.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::{AppSettings, Library, Stocks};
use oni_settings::Film;
use slint::platform::WindowEvent;
use slint::{ComponentHandle, Model};

use crate::common;

pub(crate) fn a_rolls_stock_is_picked_by_brand_and_shown_with_its_artwork() {
    let root = common::Root::new("stocks");
    let folder = root.join("Scans");
    for (name, tone) in [("A.tif", 0.8), ("B.tif", 0.5)] {
        common::linear_tiff(&folder.join(name), 96, 64, tone);
    }
    oni_library::folders::set_roll(Some(&Film::new("Roll 9")), &oni_library::paths::canonical(&folder)).unwrap();
    let Some(app) = common::launch(root).show() else { return };
    let common::App { rig, shell, .. } = &app;
    let w = &shell.window;
    app.read(&folder);
    let (lib, stocks) = (w.global::<Library>(), w.global::<Stocks>());
    lib.invoke_tile_clicked(0, false, false);
    rig.pump();
    // None picked: the first photo's mode's default, said so.
    assert_eq!(lib.get_stock().name.as_str(), "Generic Colour Negative");
    assert!(lib.get_stock().note.starts_with("Default, from this roll's"), "{}", lib.get_stock().note);
    // In French the mode is named as its tabs name it, never in English inside a French sentence.
    let settings = w.global::<AppSettings>();
    settings.invoke_language_picked(2);
    rig.pump();
    assert_eq!(lib.get_stock().note.as_str(), "Par défaut, d’après le mode Négatif de cette pellicule");
    // Twin: the mode's English name inside the French sentence.
    assert!(!lib.get_stock().note.contains("Negative"));
    settings.invoke_language_picked(1);
    rig.pump();
    assert_eq!(lib.get_stock().note.as_str(), "Default, from this roll's Negative mode", "English again");

    // The sheet: every brand first in its filter, then the brands in the catalogue's order, nothing marked.
    lib.invoke_select_roll_type();
    rig.pump();
    assert!(stocks.get_shown());
    let brands = stocks.get_brands();
    let names: Vec<String> = (0..brands.row_count()).filter_map(|i| brands.row_data(i)).map(|b| b.split(" · ").next().unwrap().to_string()).collect();
    assert_eq!(&names[..4], ["Every Brand", "Generic", "AgfaPhoto", "CineStill"]);
    assert_eq!(stocks.get_brand(), 0, "on every brand");
    let cells = every_cell(&stocks);
    // Every stock offered: no Foma 401, no line its stocks replace.
    assert_eq!(cells.len(), oni_settings::film::offered_stocks().len());
    assert!(!cells.iter().any(|c| c.name == "Foma Fomapan 401"), "Foma 401 is not offered");
    assert!(cells.iter().any(|c| c.name == "Foma Fomapan 400"), "twin: Foma 400 still is");
    for name in ["Kodak Portra 160", "Kodak Portra 400", "Kodak Portra 800", "Kodak Gold 200", "Kodak Vision3 500T", "Fujifilm C200", "Fujifilm Superia X-TRA 400"] {
        assert!(cells.iter().any(|c| c.name == name), "{name} listed");
    }
    assert!(!cells.iter().any(|c| c.name == "Kodak Portra" || c.name == "Fujifilm Fujicolor"), "a line its stocks replace is not offered");
    assert!(cells.iter().all(|c| c.has_artwork && !c.picked && c.detail != "Stock not specified"));
    rig.settle();
    println!("shot {}", rig.shot("stock-picker").display());

    // Escape and Done close it unchanged.
    rig.key("\u{1b}");
    assert!(!stocks.get_shown());
    lib.invoke_select_roll_type();
    stocks.invoke_chosen(-1);
    rig.pump();
    assert!(lib.get_stock().note.starts_with("Default"), "twin: Done picks nothing");

    // A roll on the Portra line, as one set before the split: its name and canister, its stock said unstated.
    let line = oni_settings::film::stocks().iter().position(|k| k.id == "kodak-portra").unwrap();
    lib.invoke_select_roll_type();
    stocks.invoke_chosen(line as i32);
    rig.pump();
    assert!(!stocks.get_shown());
    assert_eq!((lib.get_stock().name.as_str(), lib.get_stock().note.as_str()), ("Kodak Portra", "Stock not specified"));
    assert!(lib.get_stock().has_artwork);
    // Reopened, the line is shown marked among its stocks, at its catalogue place.
    lib.invoke_select_roll_type();
    rig.pump();
    let cells = every_cell(&stocks);
    assert_eq!(cells.len(), oni_settings::film::offered_stocks().len() + 1, "the roll's line shown beside the offered stocks");
    // The filter counts what the grid shows, the roll's line with it: every brand's, and Kodak's.
    let brands: Vec<String> = stocks.get_brands().iter().map(|b| b.to_string()).collect();
    let kodak = cells.iter().filter(|c| c.detail == "Kodak" || c.name.starts_with("Kodak ")).count();
    assert!(brands[0].ends_with(&format!(" · {}", cells.len())) && brands.contains(&format!("Kodak · {kodak}")), "{brands:?}");
    let at = cells.iter().position(|c| c.name == "Kodak Portra").expect("the roll's line is shown");
    assert!(cells[at].picked && cells[at].has_artwork && cells[at].detail == "Stock not specified", "{:?}", cells[at].detail);
    assert_eq!(cells[at + 1].name, "Kodak Portra 160");
    assert_eq!(cells.iter().filter(|c| c.picked).count(), 1);
    // Scrolled down to Kodak, for the shot.
    rig.window.dispatch_event(WindowEvent::PointerScrolled { position: slint::LogicalPosition::new(640.0, 500.0), delta_x: 0.0, delta_y: -1100.0 });
    rig.settle();
    println!("shot {}", rig.shot("stock-picker-line").display());

    // Portra 160 picked: the precise stock named, nothing left to say, the line gone from the sheet.
    let precise = oni_settings::film::stocks().iter().position(|k| k.id == "kodak-portra160").unwrap();
    stocks.invoke_chosen(precise as i32);
    rig.pump();
    assert_eq!((lib.get_stock().name.as_str(), lib.get_stock().note.as_str()), ("Kodak Portra 160", ""));
    lib.invoke_select_roll_type();
    rig.pump();
    let cells = every_cell(&stocks);
    let marked: Vec<String> = cells.iter().filter(|c| c.picked).map(|c| c.name.to_string()).collect();
    assert_eq!(marked, ["Kodak Portra 160"]);
    assert!(!cells.iter().any(|c| c.name == "Kodak Portra"), "twin: once a stock is picked the line is no longer shown");
    let every = stocks.get_brands().row_data(0).unwrap();
    assert!(every.ends_with(&format!(" · {}", cells.len())), "twin: the filter counts the line no more: {every}");
    rig.key("\u{1b}");
}

/// Every cell the sheet shows, brand after brand.
fn every_cell(stocks: &Stocks) -> Vec<oni_app::StockCell> {
    let rows = stocks.get_rows();
    (0..rows.row_count())
        .filter_map(|r| rows.row_data(r))
        .flat_map(|r| (0..r.cells.row_count()).filter_map(move |c| r.cells.row_data(c)))
        .collect()
}

/// Select Film Stock searches as The Lens DB's sheet does: the words typed leave the stocks holding each of them in
/// their brand or name, the brand filter beside them one brand's; words and a brand that leave none say so, and the sheet
/// opens again on every stock.
pub(crate) fn the_sheet_searches_its_stocks_within_the_brand_chosen() {
    let root = common::Root::new("stock-sheet");
    let folder = root.join("Scans");
    common::linear_tiff(&folder.join("A.tif"), 96, 64, 0.6);
    oni_library::folders::set_roll(Some(&Film::new("Roll 9")), &oni_library::paths::canonical(&folder)).unwrap();
    let Some(app) = common::launch(root).show() else { return };
    let common::App { rig, shell, .. } = &app;
    let w = &shell.window;
    app.read(&folder);
    let (lib, stocks) = (w.global::<Library>(), w.global::<Stocks>());
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_select_roll_type();
    rig.settle();
    let all = every_cell(&stocks).len();
    let brand = |name: &str| {
        let brands = stocks.get_brands();
        (0..brands.row_count()).find(|&i| brands.row_data(i).is_some_and(|b| b.starts_with(&format!("{name} · ")))).expect("the brand is listed") as i32
    };
    let empty = "No film stock of this brand matches these words.";
    // The search field holds the keyboard as the sheet opens: the words typed reach it.
    rig.key("kodak portra");
    rig.pump();
    let names: Vec<String> = every_cell(&stocks).iter().map(|c| c.name.to_string()).collect();
    assert!(!names.is_empty() && names.iter().all(|n| n.contains("Portra")), "the Portras alone: {names:?}");
    assert!(names.iter().any(|n| n == "Kodak Portra 400") && !names.iter().any(|n| n == "Fujifilm C200"), "twin: another film is left out: {names:?}");
    assert!(common::placed(w, Role::Text, empty).is_empty(), "twin: no empty sheet's words while stocks show");
    rig.settle();
    println!("shot {}", rig.shot("stock-sheet-search").display());
    // Ilford chosen with the words kept: none left, and the sheet says so; every brand again brings the Portras back.
    stocks.invoke_brand_chosen(brand("Ilford"));
    rig.pump();
    assert!(every_cell(&stocks).is_empty() && stocks.get_brand() == brand("Ilford"), "the words find no Ilford");
    assert_eq!(common::placed(w, Role::Text, empty).len(), 1, "the empty sheet says why");
    rig.settle();
    println!("shot {}", rig.shot("stock-sheet-empty").display());
    stocks.invoke_brand_chosen(0);
    rig.pump();
    assert_eq!(every_cell(&stocks).len(), names.len(), "twin: every brand, the Portras again");
    // Kodak chosen and the words erased in the field: every Kodak stock, as many as its line in the filter counts.
    stocks.invoke_brand_chosen(brand("Kodak"));
    for _ in "kodak portra".chars() {
        rig.key("\u{8}");
    }
    rig.pump();
    let kodak = stocks.get_brands().row_data(brand("Kodak") as usize).unwrap();
    let cells = every_cell(&stocks);
    assert!(cells.iter().all(|c| c.detail == "Kodak") && kodak.ends_with(&format!(" · {}", cells.len())), "{kodak}: {} cards", cells.len());
    rig.settle();
    println!("shot {}", rig.shot("stock-sheet-brand").display());
    // Closed and opened again: every stock, every brand, the field empty for the next words.
    rig.key("\u{1b}");
    lib.invoke_select_roll_type();
    rig.settle();
    assert!(every_cell(&stocks).len() == all && stocks.get_brand() == 0, "the sheet opens on every stock");
    rig.key("hp5");
    rig.pump();
    let found: Vec<String> = every_cell(&stocks).iter().map(|c| c.name.to_string()).collect();
    assert!(!found.is_empty() && found.iter().all(|n| n.contains("HP5")), "the field emptied at the opening: {found:?}");
    rig.key("\u{1b}");
}
