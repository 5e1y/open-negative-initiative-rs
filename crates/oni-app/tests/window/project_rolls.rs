//! A drop in a project shown roll by roll, as a hand makes it in the tree and the grid: it lands where
//! its line shows, a roll's end included, writes the order shown, and never leaves its project unbidden.

use oni_app::app;
use oni_library::paths;
use slint::Model;

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

/// Row `from` of the projects' tree, as last drawn, pressed and moved onto the top strip of row
/// `onto`: whether its line showed there, the drop then let go.
fn tree_line(hand: &Hand, from: usize, onto: usize) -> bool {
    hand.rig.drawn();
    let rows: Vec<oni_app::TreeRow> = hand.lib().get_project_rows().iter().collect();
    let start = (rows[from].x + rows[from].width / 2.0, rows[from].y + rows[from].height / 2.0);
    let to = (rows[onto].x + rows[onto].width / 2.0, rows[onto].y + 1.0);
    hand.lib().invoke_project_row_dragged(from as i32, start.0, start.1, to.0, to.1);
    hand.rig.pump();
    let line = hand.lib().get_project_drag().line;
    hand.lib().invoke_project_drag_ended(true);
    hand.rig.pump();
    line
}

/// Client stores Roll 10's A C, Roll 8's A B, then Roll 10's D, so Roll 10 shows A C D; Other holds
/// Roll 9's X. Each drop lands at its line and the file takes the order shown.
pub(crate) fn a_drop_by_roll_lands_at_its_line_in_the_tree_and_the_grid() {
    let hand = Hand::made("rolls", &FILES, |root| {
        let at = |f: &str| paths::join(&paths::canonical(&root.join("Scans")), f);
        let client: Vec<String> = ["Roll 10/A.tif", "Roll 10/C.tif", "Roll 8/A.tif", "Roll 8/B.tif", "Roll 10/D.tif"].map(at).to_vec();
        let project = |name: &str, created: f64, members: Vec<String>| {
            serde_json::json!({"id": oni_settings::Id::new().to_string(), "name": name, "description": "", "createdAt": created, "members": members})
        };
        let file = serde_json::json!({"version": 1, "projects": [project("Client", 8e8, client), project("Other", 7e8, vec![at("Roll 9/X.tif")])]});
        std::fs::create_dir_all(root.join("Support")).unwrap();
        std::fs::write(oni_library::project::file_path(&root.join("Support")), serde_json::to_vec(&file).unwrap()).unwrap();
    });
    let [a, c, d, a8, b8, x] = FILES.map(|(f, _)| hand.at(f));
    hand.open("");
    hand.lib().invoke_surface_picked(1);
    hand.rig.pump();
    assert_eq!(hand.shown(), [a.clone(), c.clone(), d.clone(), a8.clone(), b8.clone(), x.clone()], "Roll 10 shows A C D");
    hand.lib().invoke_project_row_folded(0);
    hand.rig.pump();
    assert_eq!(hand.project_titles(), ["Client", "Roll 10", "A.tif", "C.tif", "D.tif", "Roll 8", "A.tif", "B.tif", "Other"]);

    // The tree: A on the line above Roll 8's heading ends Roll 10, which reads C D A.
    assert!(tree_line(&hand, 2, 5), "the line shows above Roll 8's heading");
    assert_eq!(hand.members(), [vec![c.clone(), d.clone(), a.clone(), a8.clone(), b8.clone()], vec![x.clone()]], "the order shown, written");
    // Twin: Roll 8's B on that line would stand in Roll 10, where it cannot: no line, nothing moves.
    assert!(!tree_line(&hand, 7, 5));
    assert_eq!(hand.members()[0], [c.clone(), d.clone(), a.clone(), a8.clone(), b8.clone()]);

    // The grid: C past A, Roll 10's last tile, ends Roll 10 there.
    hand.drag(&c, past(&hand, &a));
    assert_eq!(hand.shown()[..3], [d.clone(), a.clone(), c.clone()], "Roll 10 reads D A C");
    assert_eq!(hand.members()[0], [d.clone(), a.clone(), c.clone(), a8.clone(), b8.clone()]);
    // Twin: Roll 8's A beside Roll 10's D lands nowhere.
    hand.drag(&a8, hand.before(&d));
    assert_eq!(hand.members()[0], [d.clone(), a.clone(), c.clone(), a8.clone(), b8.clone()]);

    // Past Client's last tile, A stays in Client, though Other's X is the next tile.
    let order = hand.shell.ctl.state.borrow().order().to_vec();
    assert_eq!(order[order.iter().position(|p| *p == b8).unwrap() + 1], x, "twin: the tile after the line is Other's");
    hand.drag(&a8, past(&hand, &b8));
    assert_eq!(hand.members(), [vec![d, a, c, b8, a8], vec![x]], "Roll 8 reads B A, Other untouched");
    assert!(!hand.lib().get_drop_shown(), "the line leaves with the drop");
}
