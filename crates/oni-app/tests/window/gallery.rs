//! The gallery on the headless window, driven by real pointer and key events: picks, opening, the
//! medal picker, the cell menu, projects, the archive that moves nothing, the ONI Trash, the sidebar.

use oni_app::app::{Options, Shell};
use oni_app::{App, Confirm, Library, Menus, Presets};
use oni_core::{Archive, EditorStore};
use oni_library::index::LibraryIndex;
use oni_settings::{Film, Medal};
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, Model, SharedString};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

mod hand;
use hand::Hand;


pub(crate) fn the_gallery_picks_opens_labels_and_organises_as_swift_and_the_archive_moves_nothing_by_hand() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the gallery is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-gallery-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for (name, tone) in [("A.tif", 1.0), ("B.tif", 0.8), ("C.tif", 0.6), ("D.tif", 0.4)] {
        common::linear_tiff(&folder.join(name), 96, 64, tone);
    }
    let canonical = |n: &str| oni_library::paths::canonical(&folder.join(n));
    oni_library::folders::set_roll(Some(&Film::new("Roll 7")), &oni_library::paths::canonical(&folder)).unwrap();
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support: support.clone(), caches: caches.clone(), picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(store, engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let lib = w.global::<Library>();
    let hand = Hand { rig: &rig, w, popup: Default::default() };
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    rig.settle();
    assert_eq!(hand.shown(), ["A", "B", "C", "D"]);
    assert_eq!(hand.status(), "4 photos");
    // The status line starts at the card's content inset, where the grid's first column does.
    let first = oni_app::app::tiles(w).iter().map(|t| t.x).fold(f32::MAX, f32::min);
    let line = common::placed(w, i_slint_backend_testing::AccessibleRole::Text, "4 photos").into_iter().max_by(|a, b| a.y.total_cmp(&b.y)).expect("the status line");
    let on_column = |inset: f32| (inset - first).abs() < 1.0;
    assert!(on_column(line.x - lib.get_grid_x()), "the status line starts at {} in the card, the grid at {first}", line.x - lib.get_grid_x());
    // Twin: a line 8 pt in, as the bar's own padding laid it, misses the column.
    assert!(!on_column(8.0));
    let row = |p: &str| LibraryIndex::open(&LibraryIndex::default_path(&caches)).unwrap().all().into_iter().find(|e| e.path == p).unwrap();

    // `ListPicking`: a plain click selects, ⌘ adds, ⇧ runs from the anchor ⌘ moved.
    hand.click("A", &[]);
    assert_eq!(hand.selected(), ["A"]);
    assert_eq!(hand.status(), "A.tif", "one photo names itself");
    hand.click("C", &[Key::Control]);
    assert_eq!(hand.selected(), ["A", "C"]);
    assert_eq!(hand.status(), "2 selected");
    hand.click("D", &[Key::Shift]);
    assert_eq!(hand.selected(), ["A", "C", "D"], "⇧ runs from C, the anchor ⌘ set, and keeps A");
    let mut lit = hand.lit_rows();
    lit.sort();
    assert_eq!(lit, ["A", "C", "D"], "`TreeEngine.marked`: the sidebar lights the gallery's selection");
    hand.rest();
    hand.click("B", &[]);
    assert_eq!(hand.selected(), ["B"], "twin: a plain click replaces the selection");
    assert_eq!(hand.lit_rows(), ["B"], "and the sidebar follows it");
    hand.click("B", &[Key::Control]);
    assert!(hand.selected().is_empty());
    assert_eq!(hand.lit_rows(), ["B"], "twin: ⌘ empties the selection, the ring stays on B in the sidebar");
    hand.click("B", &[Key::Control]);
    // Under the last row, the ground: a click there drops the picks, as one between the cells does.
    let below = (lib.get_grid_x() + 8.0, lib.get_grid_y() + lib.get_content_height() - lib.get_scroll() + 8.0);
    assert_eq!(hand.selected(), ["B"], "twin: B picked before the click");
    hand.press_at(below, PointerEventButton::Left);
    hand.release_at(below, PointerEventButton::Left);
    assert!(hand.selected().is_empty(), "a click under the rows drops the picks");
    // So does one in the gutter between two cells of a row, which the nearest cell no longer takes.
    hand.click("B", &[Key::Control]);
    assert_eq!(hand.selected(), ["B"], "twin: B picked again");
    let (b, c) = (hand.centre("B"), hand.centre("C"));
    assert!((b.1 - c.1).abs() < 1.0, "B and C share a row: {b:?} {c:?}");
    let gutter = ((b.0 + c.0) / 2.0, b.1);
    hand.press_at(gutter, PointerEventButton::Left);
    hand.release_at(gutter, PointerEventButton::Left);
    assert!(hand.selected().is_empty(), "a click in the gutter drops the picks");
    hand.click("B", &[Key::Control]);

    // The second plain click on the same cell within the interval opens it; on another cell it does not.
    hand.rest();
    hand.click("A", &[]);
    hand.click("B", &[]);
    assert!(w.global::<App>().get_gallery(), "two cells clicked are no double click");
    hand.click("B", &[]);
    assert!(!w.global::<App>().get_gallery());
    assert_eq!(ctl.state.borrow().open_path(), Some(canonical("B.tif").as_str()));
    rig.chord(Key::Control, "g");
    assert!(w.global::<App>().get_gallery());
    hand.rest();

    // The arrows walk the cursor, ⇧ extends from the anchor, ⌘A takes the roll, Return opens.
    hand.click("A", &[]);
    let right: SharedString = Key::RightArrow.into();
    rig.key(&right);
    assert_eq!(hand.selected(), ["B"]);
    rig.chord(Key::Shift, &right);
    assert_eq!(hand.selected(), ["B", "C"], "⇧→ extends from B");
    rig.chord(Key::Control, "a");
    assert_eq!(hand.selected(), ["A", "B", "C", "D"]);
    let ret: SharedString = Key::Return.into();
    rig.key(&ret);
    assert!(!w.global::<App>().get_gallery());
    assert_eq!(ctl.state.borrow().open_path(), Some(canonical("C.tif").as_str()), "Return opens the cursor, C");
    assert_eq!(hand.lit_rows(), ["A", "B", "C", "D"], "⌘A's four open together and stay the selection");
    rig.until("the four together", std::time::Duration::from_secs(30), || w.global::<oni_app::Canvas>().get_status().as_str() == "4 frames together");
    rig.chord(Key::Control, "g");
    hand.rest();

    // The status bar's medal picker: one photo toggles its own medal off, several keep it.
    hand.click("A", &[]);
    assert!(lib.get_medal_shown());
    // Named, the medals lead the line where it starts, the photo's name right after the bronze.
    use i_slint_backend_testing::AccessibleRole::{Button, Text};
    let lowest_of = |role, text: &str| common::placed(w, role, text).into_iter().max_by(|a, b| a.y.total_cmp(&b.y)).unwrap_or_else(|| panic!("{text} shown"));
    let lowest = |text: &str| lowest_of(Text, text);
    let (rating, name) = (lowest("Rating"), lowest("A.tif"));
    // The bar's keys, under the tile's own beneath the hand.
    let medals = ["Gold", "Silver", "Bronze"].map(|m| lowest_of(Button, m));
    assert!(on_column(rating.x - lib.get_grid_x()), "the group's name starts the line, at {}", rating.x - lib.get_grid_x());
    assert!(medals.iter().all(|m| rating.level_with(m) && name.level_with(m)), "one row: {rating:?} {medals:?} {name:?}");
    assert!(rating.x + rating.width <= medals[0].x && medals.windows(2).all(|m| m[0].x < m[1].x), "Rating, then gold, silver, bronze");
    // The status bar's inset parts the bronze's key from the name.
    let by_name = |from: f32| name.x - from <= 24.5;
    assert!(by_name(medals[2].x + medals[2].width), "the name {} pt after the bronze", name.x - medals[2].x - medals[2].width);
    // Twin: from the group's start, the name stands the whole group away.
    assert!(!by_name(rating.x));
    lib.invoke_medal_picked(1);
    rig.pump();
    assert_eq!(row(&canonical("A.tif")).label, Medal::Gold, "the first place is gold");
    assert_eq!(lib.get_medal_current(), 1);
    lib.invoke_medal_picked(1);
    rig.pump();
    assert_eq!(row(&canonical("A.tif")).label, Medal::None, "the lit medal tapped on one photo clears it");
    hand.click("B", &[Key::Control]);
    // The line's words change under the medals, which stand still.
    assert_eq!(lowest_of(Button, "Gold").x, medals[0].x, "the medals keep their place");
    // Twin: the line did change.
    assert_ne!(lowest(&hand.status()).width, name.width, "{} reads as wide as A.tif", hand.status());
    lib.invoke_medal_picked(2);
    rig.pump();
    lib.invoke_medal_picked(2);
    rig.pump();
    assert_eq!((row(&canonical("A.tif")).label, row(&canonical("B.tif")).label), (Medal::Silver, Medal::Silver), "twin: several keep it");
    // A medal laid is a gallery step: one undo takes both back, a redo lays them again.
    assert_eq!(w.global::<Menus>().get_titles().undo.as_str(), "Undo Medal on 2 photos");
    ctl.act(oni_app::Act::Undo);
    rig.pump();
    assert_eq!((row(&canonical("A.tif")).label, row(&canonical("B.tif")).label), (Medal::None, Medal::None));
    ctl.act(oni_app::Act::Redo);
    rig.pump();
    assert_eq!((row(&canonical("A.tif")).label, row(&canonical("B.tif")).label), (Medal::Silver, Medal::Silver));

    // The cell menu counts its scope, and New Project takes the two photos.
    hand.right_click("B");
    assert_eq!(hand.selected(), ["A", "B"], "a right click inside the selection keeps it whole");
    let menu = hand.menu();
    // Every line of a right click wears its glyph; twin: one line without it is seen.
    let lines: Vec<oni_app::MenuLine> = lib.get_menu_lines().iter().collect();
    assert!(common::glyphed(lines.clone()), "a line without its glyph: {menu:?}");
    let bare = lines.iter().enumerate().map(|(i, l)| if i == 1 { oni_app::MenuLine { glyph: oni_app::MenuGlyph::None, ..l.clone() } } else { l.clone() });
    assert!(!common::glyphed(bare), "twin: a line without its glyph is seen");
    let live = |text: &str| lib.get_menu_lines().iter().find(|l| l.text == text).is_some_and(|l| l.enabled);
    assert_eq!(menu[..2], ["Copy Settings\u{2026}", "Paste Settings (2 photos)"]);
    assert!(!live("Copy Settings\u{2026}") && !live("Paste Settings (2 photos)"), "Copy takes one photo, and nothing is copied yet");
    assert!(menu.contains(&"New Project from 2 Photos".to_string()) && menu.contains(&"Move 2 Photos to ONI Trash".to_string()), "{menu:?}");
    assert!(!menu.contains(&"Add to Project".to_string()), "no project yet, so no submenu");
    hand.choose("New Project from 2 Photos");
    let file = oni_library::project::load(&support);
    assert_eq!(file.projects.len(), 1);
    assert_eq!(file.projects[0].name, "Project 1");
    assert_eq!(file.projects[0].members.len(), 2);
    // A right click outside the selection selects that cell alone, the one the menu acts on.
    // The submenu opens beside the menu as the pointer rests on its line, no click; a plain line's rest opens none.
    hand.right_click("D");
    assert_eq!(hand.selected(), ["D"], "the menu's photo is the one marked");
    assert!(hand.menu().contains(&"Add to Project".to_string()));
    hand.rest_on(hand.centre("D"), "New Project from This Photo");
    assert!(hand.sub_menu().is_empty(), "twin: resting on a plain line opens no submenu");
    hand.rest_on(hand.centre("D"), "Add to Project");
    assert_eq!(hand.sub_menu(), ["Project 1"], "resting on Add to Project opens its submenu");
    assert!(hand.menu().contains(&"Add to Project".to_string()), "the menu stays beside it");
    hand.choose_sub("Project 1");
    assert_eq!(hand.selected(), ["D"], "a verb selects its scope");
    assert_eq!(oni_library::project::load(&support).projects[0].members.len(), 3);
    // Copy Settings on the menu's one photo asks what it carries, then fills the board the next menu's Paste reads.
    hand.right_click("D");
    assert!(live("Copy Settings\u{2026}"));
    hand.choose("Copy Settings\u{2026}");
    assert!(w.global::<Presets>().get_copying(), "the copy sheet asks first");
    rig.key("\n");
    assert!(!w.global::<Presets>().get_saving(), "Return copies");
    hand.right_click("A");
    assert!(live("Paste Settings"), "{:?}", hand.menu());
    hand.choose("Copy Settings\u{2026}");
    w.global::<Presets>().invoke_save_cancelled();

    // The archive moves nothing by hand: a tile dragged onto another lands nowhere.
    let names = std::fs::read_dir(&folder).unwrap().count();
    hand.drag(hand.centre("D"), (hand.centre("A").0 - 30.0, hand.centre("A").1));
    assert_eq!(hand.shown(), ["A", "B", "C", "D"]);
    assert_eq!(std::fs::read_dir(&folder).unwrap().count(), names);
    assert!(Path::new(&canonical("D.tif")).exists());

    // A project rearranges its own photos: the same drag lands D before A.
    lib.invoke_surface_picked(1);
    rig.pump();
    assert_eq!(hand.shown(), ["A", "B", "D"]);
    let rows = lib.get_project_rows();
    assert_eq!(rows.row_count(), 1);
    assert_eq!(rows.row_data(0).unwrap().title, "Project 1");
    hand.drag(hand.centre("D"), (hand.centre("A").0 - 30.0, hand.centre("A").1));
    assert_eq!(hand.shown(), ["D", "A", "B"]);
    assert!(!lib.get_ghost_shown() && !lib.get_drop_shown(), "the ghost and the line leave with the drop");
    // Twin: a press that moves under the drag distance is a click, and reorders nothing.
    hand.rest();
    let a = hand.centre("A");
    hand.drag(a, (a.0 + 2.0, a.1));
    assert_eq!(hand.shown(), ["D", "A", "B"]);
    assert_eq!(hand.selected(), ["A"]);

    // The project's info: its name, its stocks counted by photo, a description kept on blur.
    lib.invoke_project_row_clicked(0, false, false);
    rig.pump();
    assert_eq!((lib.get_info_kind(), lib.get_info_title().as_str()), (2, "Project 1"));
    assert_eq!(lib.get_stocks().row_count(), 0, "no stock set yet");
    lib.invoke_description_committed("Coast".into());
    lib.invoke_title_committed("Coast trip".into());
    rig.pump();
    let file = oni_library::project::load(&support);
    assert_eq!((file.projects[0].name.as_str(), file.projects[0].description.as_str()), ("Coast trip", "Coast"));
    lib.invoke_title_committed("   ".into());
    assert_eq!(oni_library::project::load(&support).projects[0].name, "Coast trip", "a blank name is refused");

    // Delete Project, apart from Rename…, asks, then the photos stay and the project goes.
    assert!(lib.invoke_project_row_menu(0));
    assert_eq!(hand.menu(), ["Rename…", "Delete Project"]);
    hand.choose("Delete Project");
    let confirm = w.global::<Confirm>();
    assert!(confirm.get_shown());
    assert_eq!(confirm.get_title().as_str(), "Delete \u{201c}Coast trip\u{201d}?");
    assert_eq!(confirm.get_key().as_str(), "Delete Project");
    rig.key(&ret);
    assert!(confirm.get_shown(), "the red key takes no Return");
    confirm.invoke_answered(true);
    rig.pump();
    assert!(oni_library::project::load(&support).projects.is_empty(), "twin: its click deletes");
    assert_eq!(lib.get_empty_title().as_str(), "No Photos in a Project");
    assert_eq!(lib.get_projects_empty().as_str(), "No Projects");
    lib.invoke_surface_picked(0);
    rig.pump();
    assert_eq!(hand.shown().len(), 4, "the photos are untouched");
    // File > New Project makes one from the selection, as the cell menu's line does.
    hand.rest();
    hand.click("B", &[]);
    let titled = || (w.global::<Menus>().get_titles().project.to_string(), w.global::<Menus>().get_live().new_project);
    assert_eq!(titled(), ("New Project from This Photo".to_string(), true));
    ctl.act(oni_app::Act::NewProject);
    rig.pump();
    assert_eq!(oni_library::project::load(&support).projects[0].members.len(), 1);

    // The ONI Trash: ⌘⌫ marks C, the trash row counts it, a click lays it as a grid as a folder's does.
    hand.rest();
    hand.click("C", &[]);
    let backspace: SharedString = Key::Backspace.into();
    rig.chord(Key::Control, &backspace);
    rig.landed(&ctl);
    assert_eq!(hand.shown(), ["A", "B", "D"]);
    let trash = lib.get_trash_rows();
    assert_eq!((trash.row_data(0).unwrap().title.as_str(), trash.row_data(0).unwrap().count.as_str()), ("ONI Trash", "1"));
    // What is selected names the info column, 340 pt at the window's right: the roll, then the trash.
    let heading = |name: &str| common::placed(w, i_slint_backend_testing::AccessibleRole::Text, name).iter().filter(|t| t.x >= 1280.0 - 340.0).count();
    assert_eq!((heading("Roll 7"), heading("ONI Trash")), (1, 0), "twin: the roll heads the column before the trash");
    lib.invoke_trash_row_clicked(0);
    rig.pump();
    assert_eq!((heading("Roll 7"), heading("ONI Trash")), (0, 1), "the ONI Trash heads the column, not the roll last shown");
    rig.shot("gallery-trash-info");
    assert!(lib.get_trash_rows().row_data(0).unwrap().opened);
    assert_eq!(lib.get_trash_rows().row_count(), 1, "the click opens, never folds");
    lib.invoke_trash_row_folded(0);
    rig.pump();
    assert_eq!(lib.get_trash_rows().row_count(), 2, "twin: the chevron unfolds the trash on its folder");
    assert_eq!(hand.shown(), ["C"]);
    // The folder it left titles its row and its band by its name, no account's folder in either;
    // its whole path is the row's tooltip.
    let left = oni_library::paths::parent(&canonical("C.tif"));
    let line = lib.get_trash_rows().row_data(1).unwrap();
    assert_eq!((line.title.to_string(), line.trail.to_string(), line.help.to_string()), (oni_library::paths::name(&left), String::new(), left.clone()));
    let band = lib.get_headings().row_data(0).unwrap();
    assert_eq!((band.title.to_string(), band.caption.to_string()), (oni_library::paths::name(&left), String::new()));
    assert_eq!(hand.status(), "1 photo in the ONI Trash");
    hand.click("C", &[]);
    hand.click("C", &[]);
    assert!(w.global::<App>().get_gallery(), "a tile in the ONI Trash opens nothing");
    assert_eq!(hand.status(), "1 selected");
    let menus = w.global::<Menus>();
    assert!(menus.get_live().restore);
    assert!(!titled().1, "twin: the ONI Trash makes no project");
    assert_eq!(menus.get_titles().restore.as_str(), "Restore");
    hand.right_click("C");
    assert_eq!(hand.menu(), ["Restore", common::reveal()]);
    hand.choose("Restore");
    rig.landed(&ctl);
    assert!(row(&canonical("C.tif")).trashed_at.is_none() && Path::new(&canonical("C.tif")).exists());
    assert_eq!(lib.get_empty_title().as_str(), "ONI Trash Is Empty");
    assert!(!menus.get_live().restore, "twin: nothing left to restore");
    lib.invoke_row_clicked(0, false, false);
    rig.pump();
    assert_eq!(hand.shown().len(), 4, "a row of the archive leaves the ONI Trash");

    // The roll's info writes the folder's marker: a camera body on Return, a date clamped.
    hand.rest();
    hand.click("A", &[]);
    assert_eq!((lib.get_info_kind(), lib.get_info_title().as_str()), (1, "Roll 7"));
    let marker = || oni_library::folders::roll(&oni_library::paths::canonical(&folder)).unwrap();
    lib.invoke_camera_body_committed("  Leica M6 ".into());
    rig.pump();
    assert_eq!(marker().camera_body.as_deref(), Some("Leica M6"));
    lib.invoke_shot_date_committed("4".into(), "31".into(), "2026".into());
    rig.pump();
    assert_eq!(marker().shot_date.map(oni_library::info::ymd), Some((2026, 4, 30)));
    assert!(lib.get_shot_date_set() && lib.get_day() == "30");
    lib.invoke_shot_date_committed("13".into(), "1".into(), "2026".into());
    rig.pump();
    assert_eq!(marker().shot_date.map(oni_library::info::ymd), Some((2026, 4, 30)), "twin: month 13 is refused");
    lib.invoke_shot_date_cleared();
    rig.pump();
    assert!(marker().shot_date.is_none() && !lib.get_shot_date_set());
    lib.invoke_title_committed("Roll 8".into());
    rig.pump();
    assert_eq!(marker().name, "Roll 8");
    rig.settle();
    println!("shot {}", rig.shot("gallery-organised").display());

    // ⌘G hides the sidebar with no focus leaving its fields: what they hold is written first.
    lib.set_camera_body("Nikon F3".into());
    lib.invoke_title_edited("Roll 9".into());
    rig.pump();
    assert_eq!((marker().name.as_str(), marker().camera_body.as_deref()), ("Roll 8", Some("Leica M6")), "twin: nothing is written as it is typed");
    ctl.act(oni_app::Act::ToggleMode);
    rig.pump();
    assert!(!w.global::<App>().get_gallery());
    assert_eq!((marker().name.as_str(), marker().camera_body.as_deref()), ("Roll 9", Some("Nikon F3")));
    ctl.act(oni_app::Act::ToggleMode);
    rig.pump();
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
