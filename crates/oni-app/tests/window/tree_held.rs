//! A drag held in the projects' tree while an undo lays it again one row shorter or longer: every row
//! kept keeps its element, so the hand's moves still reach the drag and its release ends it anywhere.

use i_slint_backend_testing::AccessibleRole;
use oni_app::TreeRow;
use oni_library::paths;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{LogicalPosition, Model};
use std::time::Duration;

use crate::common;
use crate::projects_tab::Hand;

const FILES: [(&str, f32); 6] =
    [("Roll 10/A.tif", 0.2), ("Roll 10/B.tif", 0.3), ("Roll 10/C.tif", 0.4), ("Roll 10/E.tif", 0.45), ("Roll 8/P.tif", 0.5), ("Roll 8/Q.tif", 0.6)];

fn row(hand: &Hand, title: &str) -> TreeRow {
    hand.lib().get_project_rows().iter().find(|r| r.title == title).unwrap_or_else(|| panic!("{title} has a row"))
}

/// The window's point of the tree's content origin: Client's row element, less its place in the tree.
fn origin(hand: &Hand) -> (f32, f32) {
    let client = row(hand, "Client");
    let placed = common::placed(&hand.shell.window, AccessibleRole::ListItem, "Client");
    let p = placed.iter().find(|p| (p.width - client.width).abs() < 0.5).expect("Client's row drawn");
    (p.x - client.x, p.y - client.y)
}

/// The window's point on the row titled `title`: its middle, or its top strip.
fn on(hand: &Hand, o: (f32, f32), title: &str, middle: bool) -> (f32, f32) {
    let r = row(hand, title);
    (o.0 + r.x + r.width / 2.0, o.1 + r.y + if middle { r.height / 2.0 } else { 1.0 })
}

fn pointer(hand: &Hand, event: WindowEvent) {
    hand.rig.window.dispatch_event(event);
    hand.rig.pump();
}

fn to(hand: &Hand, (x, y): (f32, f32)) {
    pointer(hand, WindowEvent::PointerMoved { position: LogicalPosition::new(x, y) });
}

fn pressed(hand: &Hand, at: (f32, f32), release: bool) {
    let (position, button) = (LogicalPosition::new(at.0, at.1), PointerEventButton::Left);
    pointer(hand, if release { WindowEvent::PointerReleased { position, button } } else { WindowEvent::PointerPressed { position, button } });
}

/// A press on `from`'s row, past the double click, carried in steps onto `onto`'s middle or top strip.
fn held(hand: &Hand, o: (f32, f32), from: &str, onto: &str, middle: bool) {
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    let (start, line) = (on(hand, o, from, true), on(hand, o, onto, middle));
    to(hand, start);
    pressed(hand, start, false);
    for k in 1..=4 {
        let f = k as f32 / 4.0;
        to(hand, (start.0 + (line.0 - start.0) * f, start.1 + (line.1 - start.1) * f));
    }
}

/// Where the drag's mark stands in the window, none while no drag is held.
fn mark(hand: &Hand, o: (f32, f32)) -> Option<(f32, f32)> {
    let m = hand.lib().get_project_drag();
    m.shown.then_some((o.0 + m.x, o.1 + m.y))
}

fn undo(hand: &Hand, redo: bool) {
    hand.shell.ctl.act(if redo { oni_app::Act::Redo } else { oni_app::Act::Undo });
    hand.rig.pump();
}

/// Client holds A B C, then P Q; B leaves it, then E joins it. Q held, ⌘Z takes E out, then brings B
/// back: Q's mark follows each move, and a release on no row or over the grid ends the drag.
pub(crate) fn a_held_drag_follows_the_hand_through_an_undo_that_changes_the_rows() {
    let hand = Hand::made("held-relay", &FILES, |root| {
        let at = |f: &str| paths::join(&paths::canonical(&root.join("Scans")), f);
        let members: Vec<String> = ["Roll 10/A.tif", "Roll 10/B.tif", "Roll 10/C.tif", "Roll 8/P.tif", "Roll 8/Q.tif"].map(at).to_vec();
        let client = serde_json::json!({"id": oni_settings::Id::new().to_string(), "name": "Client", "description": "", "createdAt": 8e8, "members": members});
        std::fs::create_dir_all(root.join("Support")).unwrap();
        // The file as an earlier build wrote it, imported as a launch does.
        std::fs::write(root.join("Support/projects.json"), serde_json::to_vec(&serde_json::json!({"version": 1, "projects": [client]})).unwrap()).unwrap();
        oni_app::migration::brought(&root.join("Support"), None);
    });
    let [a, b, c, e, p, q] = FILES.map(|(f, _)| hand.at(f));
    hand.open("");
    hand.click(&b, false);
    hand.menu(&b, "Remove from Project", Some("Client"));
    hand.click(&e, false);
    hand.menu(&e, "Add to Project", Some("Client"));
    let lib = hand.lib();
    lib.invoke_surface_picked(1);
    hand.rig.pump();
    lib.invoke_project_row_folded(0);
    hand.rig.drawn();
    hand.rig.pump();
    let titles = |hand: &Hand| hand.project_titles()[2..].join(" ");
    assert_eq!(titles(&hand), "A.tif C.tif E.tif Roll 8 P.tif Q.tif");
    let o = origin(&hand);
    let sorted = |hand: &Hand| {
        let mut m = hand.members().concat();
        m.sort();
        m
    };

    // Q held on P's line; ⌘Z takes E out, a row shorter: the next move still reaches the mark.
    held(&hand, o, "Q.tif", "P.tif", false);
    let line = on(&hand, o, "P.tif", false);
    assert_eq!(mark(&hand, o), Some(line), "the mark at the hand");
    undo(&hand, false);
    assert_eq!(titles(&hand), "A.tif C.tif Roll 8 P.tif Q.tif", "one row shorter under the held hand");
    let over_c = on(&hand, o, "C.tif", true);
    to(&hand, over_c);
    // Twin: the move carried the mark off the line, where a move never heard would leave it.
    assert!(over_c != line && mark(&hand, o) == Some(over_c), "the mark follows the hand: {:?}", mark(&hand, o));
    // ⌘Z again brings B back, a row longer: the mark follows still.
    undo(&hand, false);
    assert_eq!(titles(&hand), "A.tif B.tif C.tif Roll 8 P.tif Q.tif", "one row longer under the held hand");
    let over_b = on(&hand, o, "B.tif", true);
    to(&hand, over_b);
    assert_eq!(mark(&hand, o), Some(over_b), "the mark follows the hand a row lower");
    // Let go on no row: the drag ends there, dropping nothing.
    let last = row(&hand, "Q.tif");
    let below = (over_b.0, o.1 + last.y + 3.0 * last.height);
    to(&hand, below);
    pressed(&hand, below, true);
    assert_eq!(mark(&hand, o), None, "the release on no row ends the drag");
    assert_eq!(sorted(&hand), [a.clone(), b.clone(), c.clone(), p.clone(), q.clone()], "and drops nothing");

    // Q held again; ⌘⇧Z takes B out, a row shorter; let go over the grid: the drag ends there too.
    held(&hand, o, "Q.tif", "P.tif", false);
    undo(&hand, true);
    assert_eq!(titles(&hand), "A.tif C.tif Roll 8 P.tif Q.tif");
    let grid = (lib.get_grid_x() + 200.0, over_b.1);
    to(&hand, grid);
    assert_eq!(mark(&hand, o), Some(grid), "the mark follows the hand over the grid");
    pressed(&hand, grid, true);
    assert_eq!(mark(&hand, o), None, "the release over the grid ends the drag");
    assert_eq!(sorted(&hand), [a.clone(), c.clone(), p.clone(), q.clone()]);

    // E, joined again, held over Client's row; ⌘Z takes E's own row out: the drag ends with it, its mark
    // gone, so a release on no row and a click on C drop nothing, where a drag kept would let E join Client.
    undo(&hand, true);
    held(&hand, o, "E.tif", "Client", true);
    assert!(mark(&hand, o).is_some(), "twin: E's drag under way");
    undo(&hand, false);
    assert_eq!(titles(&hand), "A.tif C.tif Roll 8 P.tif Q.tif");
    assert_eq!(mark(&hand, o), None, "the drag ends with its row");
    to(&hand, below);
    pressed(&hand, below, true);
    std::thread::sleep(oni_app::system::double_click() + Duration::from_millis(50));
    let on_c = on(&hand, o, "C.tif", true);
    to(&hand, on_c);
    pressed(&hand, on_c, false);
    pressed(&hand, on_c, true);
    assert!(row(&hand, "C.tif").selected && mark(&hand, o).is_none(), "the click on C selects it");
    assert_eq!(sorted(&hand), [a, c, p, q], "E dropped nowhere");
}
