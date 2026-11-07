//! Filing photos as a hand does: New Project in its tab, named as made, renamed by Return or its menu,
//! a double click opening; a project opening at its top, a long name read whole.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::{App, Tip};
use oni_library::project;
use slint::platform::WindowEvent;
use slint::{ComponentHandle, LogicalPosition, Model};
use std::time::Duration;

use crate::common;
use crate::projects_tab::Hand;

fn renaming(hand: &Hand) -> Vec<String> {
    hand.lib().get_project_rows().iter().filter(|r| r.renaming).map(|r| r.title.to_string()).collect()
}

fn names(hand: &Hand) -> Vec<String> {
    project::load(&hand.root.join("Support")).projects.iter().map(|p| p.name.clone()).collect()
}

/// The column's New Project key, where the layout put it: left of the grid, apart from the empty
/// grid's own.
fn column_key(hand: &Hand) -> Option<common::Placed> {
    let grid = hand.lib().get_grid_x();
    common::placed(&hand.shell.window, Role::Button, "New Project").into_iter().find(|k| k.x + k.width <= grid)
}

/// A key as the window hands it over, on the window as last drawn: the monitor takes the keyboard
/// back from a field gone first.
fn typed(hand: &Hand, text: &str) {
    hand.rig.drawn();
    let w = &hand.shell.window;
    oni_app::outside::keyed(w.window(), || w.invoke_refocus());
    hand.rig.key(text);
}

/// The pointer moved away, then over `at`: the tip the element under it shows, if any.
fn tip_over(hand: &Hand, at: (f32, f32)) -> Option<String> {
    for (x, y) in [(4.0, 796.0), at] {
        hand.rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(x, y) });
        hand.rig.pump();
    }
    let tip = hand.shell.window.global::<Tip>();
    tip.get_shown().then(|| tip.get_text().to_string())
}

/// The project made from the Projects tab's key is named as it is made; Return or Rename… types
/// over its name, a double click never does, and opens a photo's row as it opens its tile.
pub(crate) fn new_project_is_made_and_named_in_its_tab_and_a_double_click_opens() {
    let hand = Hand::new("filing", &[("Roll/A.tif", 0.3), ("Roll/B.tif", 0.6)]);
    let (a, b) = (hand.at("Roll/A.tif"), hand.at("Roll/B.tif"));
    hand.open("Roll");
    let lib = hand.lib();
    // Twin: the Archive's column offers no such key.
    assert!(column_key(&hand).is_none(), "no New Project key in the Archive's column");
    lib.invoke_surface_picked(1);
    hand.rig.pump();
    let key = column_key(&hand).expect("the Projects tab's New Project key");
    common::click(&hand.rig, &hand.shell.window, key.middle());
    assert_eq!(hand.members(), [Vec::<String>::new()], "a project made, no photo in it yet");
    assert_eq!(renaming(&hand), ["Project 1"], "its name open to type as it is made");
    lib.invoke_project_row_rename_committed(0, " Client Tanaka ".into());
    hand.rig.pump();
    assert_eq!(names(&hand), ["Client Tanaka"]);

    // Filled from the Archive, the project shows its photos; the key stays beside them.
    lib.invoke_surface_picked(0);
    hand.rig.pump();
    hand.click(&a, false);
    hand.click(&b, true);
    hand.menu(&a, "Add to Project", Some("Client Tanaka"));
    lib.invoke_surface_picked(1);
    hand.rig.pump();
    assert_eq!(hand.shown(), [a.clone(), b.clone()]);
    assert!(column_key(&hand).is_some(), "New Project stays offered beside a project's photos");

    // A double click on the project is its click again, as a folder's: no field opens.
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    lib.invoke_project_row_clicked(0, false, false);
    lib.invoke_project_row_clicked(0, false, false);
    lib.invoke_project_row_double_clicked(0);
    hand.rig.pump();
    assert!(renaming(&hand).is_empty(), "a double click types over nothing");
    typed(&hand, "\n");
    assert_eq!(renaming(&hand), ["Client Tanaka"], "Return types over the project ringed");
    lib.invoke_project_row_rename_cancelled();
    hand.rig.pump();
    assert!(lib.invoke_project_row_menu(0));
    let lines: Vec<String> = lib.get_menu_lines().iter().map(|l| l.text.to_string()).collect();
    assert_eq!(lines, ["Rename…", "Delete Project"]);
    lib.invoke_menu_chosen(0);
    hand.rig.pump();
    assert_eq!(renaming(&hand), ["Client Tanaka"], "Rename… types over it");
    lib.invoke_project_row_rename_cancelled();
    hand.rig.pump();
    // Twin: with a photo picked, Return opens the photo, never the project's name.
    hand.click(&a, false);
    typed(&hand, "\n");
    assert!(renaming(&hand).is_empty() && !hand.shell.window.global::<App>().get_gallery(), "Return opens the photo picked");
    hand.shell.ctl.act(oni_app::Act::ToggleMode);
    hand.rig.pump();

    // A photo's row opens its photo on a double click, as its tile does.
    if !lib.get_project_rows().iter().any(|r| r.title == "B.tif") {
        lib.invoke_project_row_folded(0);
        hand.rig.pump();
    }
    let row = lib.get_project_rows().iter().position(|r| r.title == "B.tif").expect("B's row");
    lib.invoke_project_row_double_clicked(row as i32);
    hand.rig.pump();
    assert!(!hand.shell.window.global::<App>().get_gallery(), "the photo's row opens it");
    assert!(hand.shell.ctl.state.borrow().open_path().is_some_and(|o| o == b));
}

/// Back from the editor the open photo lands, yet a tab switched to opens at its top; a long name
/// wraps whole in the inspector and shows whole under the pointer where the column and the grid cut it.
pub(crate) fn a_project_opens_at_its_top_and_a_long_name_reads_whole() {
    let files: Vec<(String, f32)> = (0..40).map(|i| (format!("Roll/F{i:02}.tif"), 0.2 + i as f32 / 80.0)).collect();
    let tones: Vec<(&str, f32)> = files.iter().map(|(f, t)| (f.as_str(), *t)).collect();
    let hand = Hand::new("top", &tones);
    let (first, last) = (hand.at("Roll/F00.tif"), hand.at("Roll/F39.tif"));
    hand.open("Roll");
    let (lib, ctl) = (hand.lib(), hand.shell.ctl.clone());
    hand.click(&first, false);
    hand.click(&last, true);
    hand.menu(&first, "New Project from", None);
    let at = ctl.state.borrow().order().iter().position(|p| *p == last).unwrap();
    lib.invoke_tile_opened(at as i32);
    hand.rig.pump();
    ctl.act(oni_app::Act::ToggleMode);
    hand.rig.pump();
    // Twin: back from the editor, the open photo lands down the grid.
    assert!(lib.get_scroll() > 0.0, "the open photo landed: {}", lib.get_scroll());
    lib.invoke_surface_picked(1);
    hand.rig.pump();
    assert_eq!(lib.get_scroll(), 0.0, "the Projects tab opens at its top, its first row whole");

    let long = "Client Tanaka — Autumn festival delivery, rolls 8 and 10, final selects for the printed catalogue";
    assert!(lib.invoke_project_row_menu(0));
    lib.invoke_menu_chosen(0);
    hand.rig.pump();
    lib.invoke_project_row_rename_committed(0, long.into());
    hand.rig.pump();
    assert_eq!(names(&hand), [long]);
    lib.invoke_project_row_clicked(0, false, false);
    hand.rig.settle();
    let right = lib.get_grid_x() + lib.get_grid_width();
    let inspector = |name: &str| common::placed(&hand.shell.window, Role::Text, name).into_iter().find(|t| t.x >= right);
    let wrapped = inspector(long).expect("the inspector names the project");
    let panel = hand.shell.window.window().size().to_logical(1.0).width - right;
    assert!(wrapped.width <= panel, "the name stays inside the inspector: {} in {panel}", wrapped.width);
    let row = common::the(&hand.shell.window, Role::ListItem, long);
    assert_eq!(tip_over(&hand, row.middle()).as_deref(), Some(long), "the column's row, cut, shows the name whole");
    let heading = lib.get_headings().row_data(0).expect("the project's heading");
    let on_heading = (lib.get_grid_x() + 100.0, lib.get_grid_y() + heading.y - lib.get_scroll() + 12.0);
    assert_eq!(tip_over(&hand, on_heading), Some(format!("{long} / Roll")), "the heading, cut, shows it whole");

    // Twin: a short name is one line, whole in the column and the grid, and shows no tip.
    assert!(lib.invoke_project_row_menu(0));
    lib.invoke_menu_chosen(0);
    lib.invoke_project_row_rename_committed(0, "Tanaka".into());
    hand.rig.settle();
    let line = inspector("Tanaka").expect("the short name in the inspector");
    assert!(wrapped.height > 1.5 * line.height, "the long name wraps: {} against one line's {}", wrapped.height, line.height);
    let row = common::the(&hand.shell.window, Role::ListItem, "Tanaka");
    assert_eq!(tip_over(&hand, row.middle()), None, "a whole name shows no tip");
    assert_eq!(tip_over(&hand, on_heading), None, "nor a whole heading");
}
