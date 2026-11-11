//! The info sidebar's roll card on the headless window: the folder shown is a roll with nothing
//! picked, headed by its name typed over in place; its first field written makes its marker as one
//! undoable step, a focus passing through the date writes nothing, and the stock's dyes land on
//! every frame of the roll as one step.

use i_slint_backend_testing::AccessibleRole;
use oni_app::app::{Options, Shell};
use oni_app::{Library, Menus, Stocks};
use oni_core::{Archive, EditorStore};
use oni_library::paths;
use oni_settings::blocks::SeparationFamily;
use oni_settings::film::FILM_STOCKS;
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::PipelineSettings;
use slint::ComponentHandle;
use slint::platform::{Key, WindowEvent};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

/// `FieldStyle.across`: a field's padding on either side of its words.
const ACROSS: f32 = 16.0;

fn listed(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder).map(|l| l.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
    names.sort();
    names
}

fn family(path: &Path) -> Option<SeparationFamily> {
    sidecar::read(path, false, true).unwrap().settings.separation_family
}

fn stock(id: &str) -> i32 {
    FILM_STOCKS.iter().position(|k| k.id == id).unwrap() as i32
}

pub(crate) fn a_folder_is_a_roll_its_first_field_marks_and_its_stock_writes_its_dyes() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the roll card is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-roll-card-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for file in ["Scans/A.tif", "Scans/B.tif", "Scans/C.tif", "Scans/Other/D.tif", "Scans/Other/E.tif"] {
        common::linear_tiff(&root.join(file), 48, 32, 0.5);
    }
    let root = PathBuf::from(paths::canonical(&root));
    let scans = root.join("Scans");
    let other = scans.join("Other");
    let slide = PipelineSettings { separation_family: Some(SeparationFamily::E6), ..PipelineSettings::default() };
    sidecar::write(&slide, &other.join("D.tif"), WriteOptions::default()).unwrap();
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
    shell.open_folder(&scans);
    rig.until("the photos read", Duration::from_secs(30), || ctl.state.borrow().read_done() && ctl.state.borrow().order().len() == 5);
    let (lib, stocks, menus) = (w.global::<Library>(), w.global::<Stocks>(), w.global::<Menus>());
    let at = |name: &str| ctl.state.borrow().order().iter().position(|p| p.ends_with(&format!("/{name}"))).unwrap() as i32;
    let pick = |names: &[&str]| {
        std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
        lib.invoke_tile_clicked(at(names[0]), false, false);
        for name in &names[1..] {
            lib.invoke_tile_clicked(at(name), true, false);
        }
        rig.pump();
    };
    let frames = ["A.tif", "B.tif", "C.tif"].map(|n| scans.join(n));
    let families = || frames.iter().map(|p| family(p)).collect::<Vec<_>>();
    let marker = || oni_library::folders::roll(&scans.to_string_lossy());

    // Nothing picked, the folder opened is the roll shown: its card, the editor and Prepare are live.
    rig.pump();
    assert_eq!((lib.get_info_kind(), lib.get_info_title().as_str()), (1, "Scans"), "{}", lib.get_info_empty());
    let live = menus.get_live();
    assert!(live.toggle_mode && live.measure_roll && live.reset_to_default, "the open roll stands for a pick");
    assert!(w.global::<oni_app::Bar>().get_can_toggle_mode(), "the bar's mode key reads the menu's rule");
    lib.invoke_surface_picked(1);
    rig.pump();
    let live = menus.get_live();
    assert!(!live.toggle_mode && !live.measure_roll, "twin: the projects, empty, stand on no roll");
    lib.invoke_surface_picked(0);
    rig.pump();
    // The medal filter keeps the gallery, never the roll: no photo gold, the card and Prepare stay
    // on the folder opened.
    lib.invoke_filter_picked(1);
    rig.pump();
    assert!(ctl.state.borrow().order().is_empty(), "no photo is gold");
    assert_eq!((lib.get_info_kind(), lib.get_info_title().as_str()), (1, "Scans"), "{}", lib.get_info_empty());
    assert!(menus.get_live().measure_roll, "Prepare stands on the roll the filter hides");
    lib.invoke_filter_picked(0);
    rig.pump();
    rig.until("the folder told unmeasured", Duration::from_secs(10), || {
        rig.pump();
        lib.get_roll_unmeasured()
    });

    // A focus passing through the date writes nothing; a date typed writes the marker, one step.
    let shown = (lib.get_month(), lib.get_day(), lib.get_year());
    lib.invoke_shot_date_committed(shown.0.clone(), shown.1.clone(), shown.2.clone());
    rig.pump();
    assert_eq!(marker(), None, "the date as shown is no date of the roll's own");
    assert!(!menus.get_live().undo && !lib.get_shot_date_set());
    let before = listed(&scans);
    lib.invoke_shot_date_committed("4".into(), "30".into(), "2026".into());
    rig.landed(&ctl);
    assert_eq!(marker().and_then(|f| f.shot_date).map(oni_library::info::ymd), Some((2026, 4, 30)), "twin: a date typed is written");
    assert_eq!(marker().map(|f| f.name), Some("Scans".to_string()), "named after its folder");
    assert!(menus.get_live().undo && lib.get_shot_date_set());
    assert_eq!(menus.get_titles().undo.as_str(), "Undo Set Shot Date");
    let mut after = before.clone();
    after.push(".oni-roll.json".into());
    after.sort();
    assert_eq!(listed(&scans), after, "no file moves");
    ctl.act(oni_app::Act::Undo);
    rig.landed(&ctl);
    assert_eq!(marker(), None, "one undo takes the marker away");
    assert_eq!((lib.get_info_kind(), lib.get_info_title().as_str()), (1, "Scans"), "the folder stays the roll shown");

    // The roll's name heads the column, the warning under it, and Film Stock is its first block.
    let first = common::the(w, AccessibleRole::Text, "Film Stock");
    // The column's edge is the blocks' fold keys, a block's title standing past its chevron.
    let edge = common::the(w, AccessibleRole::Button, "Hide Film Stock").x;
    let in_column = |words: &str| common::placed(w, AccessibleRole::Text, words).into_iter().filter(|p| p.x >= edge - 1.0).collect::<Vec<_>>();
    let (named, warned) = (in_column("Scans"), in_column("Not prepared yet"));
    assert!(named.len() == 1 && warned.len() == 1, "{named:?} {warned:?}");
    assert!(named[0].y < warned[0].y && warned[0].y < first.y, "{named:?} over {warned:?} over {first:?}");
    assert!(in_column("Roll").is_empty(), "no block stands for the name alone");
    assert_eq!(in_column("Film Stock").len(), 1, "twin: the same search finds a block's title");

    // A name typed over the title makes the roll under it, the folder keeping its own name. The field the
    // title becomes keeps the column's margins, as wide as the column's content, its stock's key; twin: off
    // the column's edge, where its fold keys stand.
    common::click(&rig, w, named[0].middle());
    rig.settle();
    let content = common::the(w, AccessibleRole::Button, "Select Film Stock");
    let field = common::placed_all(w, AccessibleRole::TextInput).into_iter().find(|t| t.x >= edge - 1.0 && (t.y - named[0].y).abs() < 24.0).expect("the title's field");
    let (left, right) = (field.x - ACROSS, field.x + field.width + ACROSS);
    // The words' room ends a caret's point short of the face's padding.
    assert!((left - content.x).abs() < 0.5 && (right - (content.x + content.width)).abs() < 1.5, "the title's field on the column's content: {left} {right} {content:?}");
    assert!(left > edge, "twin: the field starts past the fold keys on the column's edge: {left} {edge}");
    for c in "Portra Trip".chars() {
        rig.key(&c.to_string());
    }
    rig.key(&slint::SharedString::from(Key::Return));
    rig.landed(&ctl);
    assert_eq!(marker().map(|f| f.name), Some("Portra Trip".to_string()));
    assert_eq!(lib.get_info_title().as_str(), "Portra Trip");
    assert!(scans.join("Portra Trip").symlink_metadata().is_err(), "the folder keeps its name");

    // Portra picked: Kodak's dyes on its three frames in one step with the marker, undone as one.
    lib.invoke_select_roll_type();
    rig.pump();
    assert!(stocks.get_shown());
    stocks.invoke_chosen(stock("kodak-portra"));
    rig.landed(&ctl);
    assert_eq!(marker().and_then(|f| f.stock), Some("kodak-portra".to_string()));
    assert_eq!(families(), vec![Some(SeparationFamily::KodakC41); 3], "Portra writes Kodak's dyes on every frame");
    assert_eq!(family(&other.join("D.tif")), Some(SeparationFamily::E6), "twin: a frame outside the roll keeps its family");
    rig.until("the card reads Kodak's dyes off the frames", Duration::from_secs(10), || {
        rig.pump();
        lib.get_stock_dyes().as_str() == "Dyes: Kodak"
    });
    rig.settle();
    println!("shot {}", rig.shot("roll-card-made").display());
    ctl.act(oni_app::Act::Undo);
    rig.landed(&ctl);
    assert_eq!(families(), vec![None; 3], "one undo gives the families back");
    assert_eq!(marker().map(|f| (f.name, f.stock)), Some(("Portra Trip".to_string(), None)), "and the stock");
    ctl.act(oni_app::Act::Redo);
    rig.landed(&ctl);
    assert_eq!(families(), vec![Some(SeparationFamily::KodakC41); 3]);

    // A black-and-white stock leaves every family alone, and the card says the dyes the frames
    // still unmix under a stock naming none.
    lib.invoke_select_roll_type();
    stocks.invoke_chosen(stock("ilford-hp5"));
    rig.landed(&ctl);
    assert_eq!(families(), vec![Some(SeparationFamily::KodakC41); 3], "black and white writes no dyes");
    rig.pump();
    assert_eq!(lib.get_stock_dyes().as_str(), "Dyes: Kodak");

    // A measured frame takes the banner away once the roll's frames are read again.
    let measured = frames[1].clone();
    let mut b = sidecar::read(&measured, false, true).unwrap().settings;
    b.roll_reference = Some(oni_settings::frame::RollReference { id: oni_settings::Id::new(), stops: [0.0; 3], black: [0.1; 3], white: [0.6; 3], base: None, median: None });
    sidecar::write(&b, &measured, WriteOptions::default()).unwrap();
    lib.invoke_select_roll_type();
    stocks.invoke_chosen(stock("kodak-gold"));
    rig.landed(&ctl);
    rig.until("the measured roll's banner gone", Duration::from_secs(10), || {
        rig.pump();
        !lib.get_roll_unmeasured()
    });

    // A photo of another folder picked: that folder is the roll shown, its card and no guide.
    pick(&["D.tif"]);
    assert_eq!((lib.get_info_kind(), lib.get_info_title().as_str()), (1, "Other"));

    // The frame open in the editor takes the roll's new dyes at once, its Roll tab with it.
    let renders = ctl.state.borrow().renders();
    lib.invoke_tile_opened(at("A.tif"));
    rig.until("A rendered", Duration::from_secs(120), || ctl.state.borrow().renders() > renders);
    ctl.act(oni_app::Act::ToggleMode);
    rig.pump();
    pick(&["A.tif"]);
    lib.invoke_select_roll_type();
    stocks.invoke_chosen(stock("fujifilm-superia"));
    rig.landed(&ctl);
    assert_eq!(ctl.state.borrow().store().settings().separation_family, Some(SeparationFamily::FujiC41));
    let (panel, g) = (w.global::<oni_app::PanelState>(), w.global::<oni_app::Store>());
    assert_eq!(panel.get_dye_family(), 1, "the Dyes tabs read Fuji");
    assert_eq!(panel.get_dye_source().as_str(), "From the film stock, Fujifilm Superia");
    assert!(!g.get_dye_separation_modified(), "the stock's dyes are the block's rest: no reset key");
    rig.until("the card reads Fuji's dyes", Duration::from_secs(10), || {
        rig.pump();
        lib.get_stock_dyes().as_str() == "Dyes: Fuji"
    });

    // Kodak picked on the photo open: the block flags it against the stock, the card warns with the
    // stock's dyes offered, and the block's reset lands back on the stock's dyes.
    panel.invoke_dye_family_picked(0);
    rig.pump();
    assert_eq!(ctl.state.borrow().store().settings().separation_family, Some(SeparationFamily::KodakC41));
    assert_eq!(panel.get_dye_source().as_str(), "Not the film stock's: Fujifilm Superia has Fuji dyes");
    assert!(g.get_dye_separation_modified(), "twin: off the stock's dyes, the reset key shows");
    let warned = || (lib.get_stock_dyes().to_string(), lib.get_stock_dyes_fix().to_string());
    let warning = ("Frames set to Kodak dyes, but this film stock has Fuji dyes".to_string(), "Use Fuji Dyes".to_string());
    rig.until("the card warns of the frame's Kodak under the stock's Fuji", Duration::from_secs(10), || {
        rig.pump();
        warned() == warning
    });
    rig.settle();
    println!("shot {}", rig.shot("roll-card-dyes-warned").display());

    // The warning's key, clicked: the stock's dyes on every frame as one step, the open photo's with them.
    common::click(&rig, w, common::the(w, i_slint_backend_testing::AccessibleRole::Button, "Use Fuji Dyes").middle());
    rig.landed(&ctl);
    assert_eq!(ctl.state.borrow().store().settings().separation_family, Some(SeparationFamily::FujiC41));
    rig.until("the card quiet on Fuji, its key gone", Duration::from_secs(10), || {
        rig.pump();
        warned() == ("Dyes: Fuji".to_string(), String::new())
    });
    ctl.act(oni_app::Act::Undo);
    rig.landed(&ctl);
    assert_eq!(ctl.state.borrow().store().settings().separation_family, Some(SeparationFamily::KodakC41), "twin: one undo gives the hand's Kodak back");
    rig.until("the card warning again", Duration::from_secs(10), || {
        rig.pump();
        warned() == warning
    });
    g.invoke_reset(4);
    rig.pump();
    assert_eq!(ctl.state.borrow().store().settings().separation_family, Some(SeparationFamily::FujiC41));
    assert_eq!((panel.get_dye_family(), g.get_dye_separation_modified()), (1, false));
    assert_eq!(panel.get_dye_source().as_str(), "From the film stock, Fujifilm Superia");
    rig.until("the card back on Fuji alone", Duration::from_secs(10), || {
        rig.pump();
        lib.get_stock_dyes().as_str() == "Dyes: Fuji"
    });
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
