//! A drop in a project, as a hand makes it in the tree and the grid: the project shows its photos in
//! its own order, whatever roll each lies in; a drop lands where its mark shows, only where it changes
//! something, writes the order shown, and never leaves its project unbidden.

use oni_app::app;
use oni_library::paths;
use slint::{ComponentHandle, Model};

use crate::projects_tab::Hand;

const FILES: [(&str, f32); 6] =
    [("Roll 10/A.tif", 0.2), ("Roll 10/C.tif", 0.3), ("Roll 10/D.tif", 0.4), ("Roll 8/A.tif", 0.5), ("Roll 8/B.tif", 0.6), ("Roll 9/X.tif", 0.7)];

/// Where a drop lands past `path`'s tile, a little right of its middle.
fn past(hand: &Hand, path: &str) -> (f32, f32) {
    let lib = hand.lib();
    let i = hand.shell.ctl.state.borrow().order().iter().position(|p| p == path).unwrap();
    let t = app::tiles(&hand.shell.window).into_iter().find(|t| t.index as usize == i).unwrap();
    (lib.get_grid_x() + t.x + t.width / 2.0 + 30.0, lib.get_grid_y() + t.y - lib.get_scroll() + t.height / 2.0)
}

/// Row `from` of the projects' tree, as last drawn, pressed and moved onto row `onto`, its middle or
/// its top strip: the mark shown there, the drop then let go or cancelled.
fn tree_mark(hand: &Hand, from: usize, onto: usize, middle: bool, dropped: bool) -> oni_app::TreeDrag {
    hand.rig.drawn();
    let rows: Vec<oni_app::TreeRow> = hand.lib().get_project_rows().iter().collect();
    let start = (rows[from].x + rows[from].width / 2.0, rows[from].y + rows[from].height / 2.0);
    let to = (rows[onto].x + rows[onto].width / 2.0, rows[onto].y + if middle { rows[onto].height / 2.0 } else { 1.0 });
    hand.lib().invoke_project_row_dragged(from as i32, start.0, start.1, to.0, to.1);
    hand.rig.pump();
    let mark = hand.lib().get_project_drag();
    hand.lib().invoke_project_drag_ended(dropped);
    hand.rig.pump();
    mark
}

/// `tree_mark` onto the top strip of row `onto`: whether its line showed there.
pub(crate) fn tree_line(hand: &Hand, from: usize, onto: usize) -> bool {
    tree_mark(hand, from, onto, false, true).line
}

/// Client stores Roll 10's A C, Roll 8's A B, then Roll 10's D, shown so under its name alone; Other
/// holds Roll 9's X. Each drop lands at its line, whatever its roll, and the file takes the order shown.
pub(crate) fn a_drop_lands_at_its_line_in_the_project_s_own_order_in_the_tree_and_the_grid() {
    let hand = Hand::made("order", &FILES, |root| {
        let at = |f: &str| paths::join(&paths::canonical(&root.join("Scans")), f);
        let client: Vec<String> = ["Roll 10/A.tif", "Roll 10/C.tif", "Roll 8/A.tif", "Roll 8/B.tif", "Roll 10/D.tif"].map(at).to_vec();
        let project = |name: &str, created: f64, members: Vec<String>| {
            serde_json::json!({"id": oni_settings::Id::new().to_string(), "name": name, "description": "", "createdAt": created, "members": members})
        };
        let file = serde_json::json!({"version": 1, "projects": [project("Client", 8e8, client), project("Other", 7e8, vec![at("Roll 9/X.tif")])]});
        std::fs::create_dir_all(root.join("Support")).unwrap();
        // The file as an earlier build wrote it, imported as a launch does.
        std::fs::write(root.join("Support/projects.json"), serde_json::to_vec(&file).unwrap()).unwrap();
        oni_app::migration::brought(&root.join("Support"), None);
    });
    let [a, c, d, a8, b8, x] = FILES.map(|(f, _)| hand.at(f));
    hand.open("");
    hand.lib().invoke_surface_picked(1);
    hand.rig.pump();
    assert_eq!(hand.shown(), [a.clone(), c.clone(), a8.clone(), b8.clone(), d.clone(), x.clone()], "the order stored, rolls interleaved");
    let headings: Vec<String> = hand.lib().get_headings().iter().map(|h| h.title.into()).collect();
    assert_eq!(headings, ["Client", "Other"], "a band for each project, none for a roll");
    hand.lib().invoke_project_row_folded(0);
    hand.rig.pump();
    assert_eq!(hand.project_titles(), ["Client", "A.tif", "C.tif", "A.tif", "B.tif", "D.tif", "Other"]);
    // Twin: no row of the tree heads a roll.
    assert!(hand.lib().get_project_rows().iter().all(|r| r.kind != 3));

    // The tree: Roll 10's A on the line above Roll 8's B lands there.
    assert!(tree_line(&hand, 1, 4), "the line shows above Roll 8's B");
    assert_eq!(hand.members(), [vec![c.clone(), a8.clone(), a.clone(), b8.clone(), d.clone()], vec![x.clone()]], "the order shown, written");
    // A on its own project's row or on a line beside itself changes nothing: no mark.
    let onto_client = tree_mark(&hand, 3, 0, true, true);
    assert!(!onto_client.ring && !tree_line(&hand, 3, 3) && !tree_line(&hand, 3, 4), "no mark where A stands already");
    assert_eq!(hand.members()[0], [c.clone(), a8.clone(), a.clone(), b8.clone(), d.clone()]);
    // Twin: Other's row rings, as A would join it; the line above Other ends Client, where C goes.
    assert!(tree_mark(&hand, 3, 6, true, false).ring);
    assert!(tree_line(&hand, 1, 6));
    assert_eq!(hand.members(), [vec![a8.clone(), a.clone(), b8.clone(), d.clone(), c.clone()], vec![x.clone()]]);

    // The grid: Roll 8's A past C, Client's last tile, ends Client there.
    assert!(hand.drag(&a8, past(&hand, &c)), "the line shows past C");
    assert_eq!(hand.members()[0], [a.clone(), b8.clone(), d.clone(), c.clone(), a8.clone()]);
    // Roll 8's B lands before Roll 10's A; twin: C past itself changes nothing, no line.
    assert!(hand.drag(&b8, hand.before(&a)) && !hand.drag(&c, past(&hand, &c)));
    assert_eq!(hand.members()[0], [b8.clone(), a.clone(), d.clone(), c.clone(), a8.clone()]);

    // Past Client's last tile, D stays in Client, though Other's X is the next tile.
    let order = hand.shell.ctl.state.borrow().order().to_vec();
    assert_eq!(order[order.iter().position(|p| *p == a8).unwrap() + 1], x, "twin: the tile after the line is Other's");
    hand.drag(&d, past(&hand, &a8));
    assert_eq!(hand.members(), [vec![b8, a, c, a8, d], vec![x]], "D ends Client, Other untouched");
    assert!(!hand.lib().get_drop_shown(), "the line leaves with the drop");
}

/// Client stores Roll 10's A, Roll 8's A, then Roll 10's D; under Gold, worn by Roll 8's A and Roll
/// 10's D, the grid keeps the project's order: Roll 8's A, then D.
pub(crate) fn the_rating_filter_keeps_the_project_s_order() {
    let files = [("Roll 10/A.tif", 0.2), ("Roll 10/D.tif", 0.4), ("Roll 8/A.tif", 0.5)];
    let hand = Hand::made("order-filter", &files, |root| {
        let at = |f: &str| paths::join(&paths::canonical(&root.join("Scans")), f);
        let members: Vec<String> = ["Roll 10/A.tif", "Roll 8/A.tif", "Roll 10/D.tif"].map(at).to_vec();
        let client = serde_json::json!({"id": oni_settings::Id::new().to_string(), "name": "Client", "description": "", "createdAt": 8e8, "members": members});
        std::fs::create_dir_all(root.join("Support")).unwrap();
        // The file as an earlier build wrote it, imported as a launch does.
        std::fs::write(root.join("Support/projects.json"), serde_json::to_vec(&serde_json::json!({"version": 1, "projects": [client]})).unwrap()).unwrap();
        oni_app::migration::brought(&root.join("Support"), None);
    });
    let [a, d, a8] = files.map(|(f, _)| hand.at(f));
    hand.open("");
    let lib = hand.lib();
    for gold in [&a8, &d] {
        hand.click(gold, false);
        lib.invoke_medal_picked(1);
        hand.rig.pump();
    }
    lib.invoke_surface_picked(1);
    hand.rig.pump();
    assert_eq!(hand.shown(), [a.clone(), a8.clone(), d.clone()], "the project's order");
    lib.invoke_filter_picked(1);
    hand.rig.pump();
    assert_eq!(hand.shown(), [a8.clone(), d.clone()], "Gold keeps Roll 8's A ahead");
    // Twin: Gold's photos by roll would put Roll 10's D first.
    assert_ne!(hand.shown()[0], d);
}

/// Client reads A C D once A moved before C; D held on the line above C, row 2, then ⌘Z puts back
/// C A D under the held hand: the line stands above A, and the release lands D there.
pub(crate) fn a_tree_drop_lands_at_its_line_when_the_tree_changes_under_the_hand() {
    let files = [("Roll 10/A.tif", 0.2), ("Roll 10/C.tif", 0.3), ("Roll 10/D.tif", 0.4)];
    let hand = Hand::made("rolls-held", &files, |root| {
        let at = |f: &str| paths::join(&paths::canonical(&root.join("Scans")), f);
        let members: Vec<String> = ["Roll 10/C.tif", "Roll 10/A.tif", "Roll 10/D.tif"].map(at).to_vec();
        let client = serde_json::json!({"id": oni_settings::Id::new().to_string(), "name": "Client", "description": "", "createdAt": 8e8, "members": members});
        std::fs::create_dir_all(root.join("Support")).unwrap();
        // The file as an earlier build wrote it, imported as a launch does.
        std::fs::write(root.join("Support/projects.json"), serde_json::to_vec(&serde_json::json!({"version": 1, "projects": [client]})).unwrap()).unwrap();
        oni_app::migration::brought(&root.join("Support"), None);
    });
    let [a, c, d] = files.map(|(f, _)| hand.at(f));
    hand.open("");
    let lib = hand.lib();
    lib.invoke_surface_picked(1);
    hand.rig.pump();
    lib.invoke_project_row_folded(0);
    hand.rig.pump();
    assert_eq!(hand.project_titles(), ["Client", "C.tif", "A.tif", "D.tif"]);
    assert!(tree_line(&hand, 2, 1), "A moves before C");
    assert_eq!(hand.members(), [vec![a.clone(), c.clone(), d.clone()]]);

    hand.rig.drawn();
    let rows: Vec<oni_app::TreeRow> = lib.get_project_rows().iter().collect();
    let start = (rows[3].x + rows[3].width / 2.0, rows[3].y + rows[3].height / 2.0);
    let to = (rows[2].x + rows[2].width / 2.0, rows[2].y + 1.0);
    lib.invoke_project_row_dragged(3, start.0, start.1, to.0, to.1);
    hand.rig.pump();
    let held = lib.get_project_drag();
    // Twin: before the undo, the row under the line holds C, where a landing kept from then puts D.
    assert!(held.line && hand.project_titles()[2] == "C.tif");
    hand.shell.ctl.act(oni_app::Act::Undo);
    hand.rig.pump();
    assert_eq!(hand.project_titles()[1..], ["C.tif", "A.tif", "D.tif"], "the undo puts back C A D, the hand still held");
    let undone = lib.get_project_drag();
    assert!(undone.line && undone.mark_y == held.mark_y, "the line stays on row 2, now above A");
    lib.invoke_project_row_dragged(3, start.0, start.1, to.0, to.1 + 1.0);
    hand.rig.pump();
    lib.invoke_project_drag_ended(true);
    hand.rig.pump();
    assert_eq!(hand.members(), [vec![c, d, a]], "D lands above A, where its line stood");
}

/// Client lays Roll 8's A, Roll 10's A, then Roll 8's B in one run: Prepare the Roll asked on Roll
/// 8's A takes Roll 8's two frames, never Roll 10's, though the project shows them together.
pub(crate) fn prepare_from_a_project_takes_the_frame_s_roll_alone() {
    let files = [("Roll 10/A.tif", 0.2), ("Roll 8/A.tif", 0.5), ("Roll 8/B.tif", 0.6)];
    let hand = Hand::made("order-prepare", &files, |root| {
        let at = |f: &str| paths::join(&paths::canonical(&root.join("Scans")), f);
        let members: Vec<String> = ["Roll 8/A.tif", "Roll 10/A.tif", "Roll 8/B.tif"].map(at).to_vec();
        let client = serde_json::json!({"id": oni_settings::Id::new().to_string(), "name": "Client", "description": "", "createdAt": 8e8, "members": members});
        std::fs::create_dir_all(root.join("Support")).unwrap();
        // The file as an earlier build wrote it, imported as a launch does.
        std::fs::write(root.join("Support/projects.json"), serde_json::to_vec(&serde_json::json!({"version": 1, "projects": [client]})).unwrap()).unwrap();
        oni_app::migration::brought(&root.join("Support"), None);
    });
    let [r10a, r8a, r8b] = files.map(|(f, _)| hand.at(f));
    hand.open("");
    hand.lib().invoke_surface_picked(1);
    hand.rig.pump();
    assert_eq!(hand.shown(), [r8a.clone(), r10a.clone(), r8b.clone()], "one run, the rolls interleaved");
    hand.click(&r8a, false);
    hand.shell.ctl.act(oni_app::Act::PrepareRoll);
    let prepare = hand.shell.window.global::<oni_app::Preparing>();
    hand.rig.until("Prepare shows Roll 8", std::time::Duration::from_secs(30), || prepare.get_shown() && prepare.get_cells().row_count() > 0);
    // Twin: the project's whole band would lay three cells.
    assert_eq!(prepare.get_cells().row_count(), 2, "Roll 8's A and B alone");
}
