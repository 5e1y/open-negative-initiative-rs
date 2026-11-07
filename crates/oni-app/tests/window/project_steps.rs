//! A step on the projects' members undone and done again as a hand meets it: the project New
//! Project made goes and comes back, and a member brought back beyond the folder shows at once.

use crate::projects_tab::Hand;
use oni_library::project;

/// New Project from A and B, then ⌘Z: no project is left behind, and ⌘⇧Z makes the same one again.
pub(crate) fn an_undo_of_new_project_leaves_no_project_and_its_redo_makes_it_again() {
    let tones = [("Roll/A.tif", 0.3), ("Roll/B.tif", 0.6)];
    let hand = Hand::new("undo-new", &tones);
    let [a, b] = tones.map(|(f, _)| hand.at(f));
    hand.open("Roll");
    let ctl = hand.shell.ctl.clone();
    hand.click(&a, false);
    hand.click(&b, true);
    hand.menu(&a, "New Project from", None);
    assert_eq!(hand.members(), [vec![a.clone(), b.clone()]]);
    ctl.act(oni_app::Act::Undo);
    hand.rig.pump();
    // Twin: the members alone laid back leave the project, empty, as a leftover.
    assert_ne!(hand.members(), [Vec::<String>::new()]);
    assert!(hand.members().is_empty(), "the project the step made is gone");
    ctl.act(oni_app::Act::Redo);
    hand.rig.pump();
    assert_eq!(hand.members(), [vec![a, b]]);
    hand.lib().invoke_surface_picked(1);
    hand.rig.pump();
    assert_eq!(hand.project_titles(), ["Project 1"], "the same project, its name kept");
}

/// Z, in Roll B, leaves the project while Roll A is open, then Roll C opens: ⌘Z shows Z in the
/// project at once, read again off the index, without waiting for the next folder's read.
pub(crate) fn an_undo_shows_a_member_beyond_the_folder_at_once() {
    let tones = [("Roll A/A.tif", 0.3), ("Roll B/Z.tif", 0.6), ("Roll C/C.tif", 0.45)];
    let hand = Hand::new("undo-beyond", &tones);
    let [a, z, _] = tones.map(|(f, _)| hand.at(f));
    hand.open("");
    let ctl = hand.shell.ctl.clone();
    hand.click(&a, false);
    hand.click(&z, true);
    hand.menu(&a, "New Project from", None);
    hand.open("Roll A");
    hand.lib().invoke_surface_picked(1);
    hand.rig.pump();
    assert_eq!(hand.shown(), [a.clone(), z.clone()], "Z found beyond the folder");
    hand.click(&z, false);
    hand.menu(&z, "Remove from Project", Some("Project 1"));
    hand.open("Roll C");
    hand.lib().invoke_surface_picked(1);
    hand.rig.pump();
    // Twin: Z out of the project, the Projects tab shows A alone.
    assert_eq!(hand.shown(), [a.clone()]);
    ctl.act(oni_app::Act::Undo);
    hand.rig.pump();
    assert_eq!(hand.members(), [vec![a.clone(), z.clone()]]);
    assert_eq!(hand.shown(), [a, z], "Z shows again at once");
}

/// The Projects tab's New Project key is one step: ⌘Z takes its project away, never the step before,
/// and ⌘⇧Z brings it back under the name typed since; a project dragged above another is no step.
pub(crate) fn the_new_project_key_is_one_step_and_its_redo_keeps_the_name_typed() {
    let tones = [("Roll/A.tif", 0.3)];
    let hand = Hand::new("undo-key", &tones);
    let [a] = tones.map(|(f, _)| hand.at(f));
    hand.open("Roll");
    let (ctl, lib) = (hand.shell.ctl.clone(), hand.lib());
    hand.click(&a, false);
    hand.menu(&a, "New Project from", None);
    lib.invoke_surface_picked(1);
    hand.rig.pump();
    lib.invoke_new_project();
    hand.rig.pump();
    lib.invoke_project_row_rename_committed(0, "Client".into());
    hand.rig.pump();
    let names = |hand: &Hand| project::load(&hand.root.join("Support")).projects.iter().map(|p| p.name.clone()).collect::<Vec<_>>();
    assert_eq!(names(&hand), ["Project 1", "Client"]);
    ctl.act(oni_app::Act::Undo);
    hand.rig.pump();
    assert_eq!((names(&hand), hand.members()), (vec!["Project 1".to_string()], vec![vec![a.clone()]]), "the key's project gone, the step before kept");
    ctl.act(oni_app::Act::Redo);
    hand.rig.pump();
    assert_eq!(names(&hand), ["Project 1", "Client"], "back under the name typed since");

    let titles = hand.project_titles();
    let at = |title: &str| titles.iter().position(|t| t == title).unwrap();
    assert!(crate::project_rolls::tree_line(&hand, at("Project 1"), at("Client")), "the line shows above Client");
    assert!(hand.project_titles().iter().position(|t| t == "Project 1") < hand.project_titles().iter().position(|t| t == "Client"));
    ctl.act(oni_app::Act::Undo);
    hand.rig.pump();
    // Twin: had the drag been a step, this ⌘Z would have taken it back and left Client.
    assert_eq!(names(&hand), ["Project 1"], "the drag no step, ⌘Z takes back the key's project");
}
