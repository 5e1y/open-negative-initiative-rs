//! Select Roll Type on the headless window: the stocks by brand with their artwork, a click picking
//! the roll's stock and closing, Done and Escape changing nothing.

use oni_app::app::{Options, Shell};
use oni_app::{Library, Stocks};
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
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), oni_app::render::Engine::shared(&gpu), options).unwrap();
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
    assert_eq!(lib.get_stock().as_str(), "Generic Color Negative");
    assert!(lib.get_stock_note().starts_with("Default, from this roll's"), "{}", lib.get_stock_note());

    // The sheet: the brands in the catalogue's order, nothing marked.
    lib.invoke_select_roll_type();
    rig.pump();
    assert!(stocks.get_shown());
    let brands = stocks.get_brands();
    let names: Vec<String> = (0..brands.row_count()).filter_map(|i| brands.row_data(i)).map(|b| b.brand.to_string()).collect();
    assert_eq!(&names[..3], ["GENERIC", "AGFAPHOTO", "CINESTILL"]);
    let cells: Vec<_> = (0..brands.row_count())
        .filter_map(|i| brands.row_data(i))
        .flat_map(|b| (0..b.rows.row_count()).filter_map(move |r| b.rows.row_data(r)))
        .flat_map(|r| (0..r.cells.row_count()).filter_map(move |c| r.cells.row_data(c)))
        .collect();
    assert_eq!(cells.len(), FILM_STOCKS.len());
    assert!(cells.iter().all(|c| c.has_artwork && !c.picked));
    rig.settle();
    println!("shot {}", rig.shot("stock-picker").display());

    // Escape and Done close it unchanged.
    rig.key("\u{1b}");
    assert!(!stocks.get_shown());
    lib.invoke_select_roll_type();
    stocks.invoke_chosen(-1);
    rig.pump();
    assert!(lib.get_stock_note().starts_with("Default"), "twin: Done picks nothing");

    // A click picks Portra for the roll, shown with its artwork; reopened, Portra is marked.
    let portra = FILM_STOCKS.iter().position(|k| k.id == "kodak-portra").unwrap();
    lib.invoke_select_roll_type();
    stocks.invoke_chosen(portra as i32);
    rig.pump();
    assert!(!stocks.get_shown());
    assert_eq!((lib.get_stock().as_str(), lib.get_stock_note().as_str()), ("Kodak Portra", ""));
    assert!(lib.get_stock_has_artwork());
    lib.invoke_select_roll_type();
    rig.pump();
    let brands = stocks.get_brands();
    let marked: Vec<String> = (0..brands.row_count())
        .filter_map(|i| brands.row_data(i))
        .flat_map(|b| (0..b.rows.row_count()).filter_map(move |r| b.rows.row_data(r)))
        .flat_map(|r| (0..r.cells.row_count()).filter_map(move |c| r.cells.row_data(c)))
        .filter(|c| c.picked)
        .map(|c| c.name.to_string())
        .collect();
    assert_eq!(marked, ["Kodak Portra"]);
    rig.key("\u{1b}");
    let _ = std::fs::remove_dir_all(&root);
}
