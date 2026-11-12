//! A launch whose import failed: the favourites and projects shown as the old files hold them, each
//! change taken back and said, nothing written under `State/`, so the next launch brings them.

use crate::projects_tab::Hand;
use oni_app::migration::{Brought, NOT_BROUGHT};
use oni_app::{Navigator, Told};
use oni_library::{paths, places::Places, project, state};
use slint::{ComponentHandle, Model};

/// The navigator's row titled so, and whether it is starred.
fn starred_row(hand: &Hand, title: &str) -> Option<(usize, bool)> {
    let rows = hand.shell.window.global::<Navigator>().get_rows();
    (0..rows.row_count()).filter_map(|i| rows.row_data(i).map(|l| (i, l))).find(|(_, l)| l.title == title).map(|(i, l)| (i, l.favourite))
}

pub(crate) fn an_import_due_is_shown_from_the_old_files_and_never_written() {
    let tones = [("Roll/A.tif", 0.3), ("Roll/B.tif", 0.6)];
    let due = Brought { places_due: true, projects_due: true, notice: Some(NOT_BROUGHT) };
    let hand = Hand::made_with(
        "due",
        &tones,
        |root| {
            let prints: Vec<String> = tones.iter().map(|(f, _)| oni_library::relink::fingerprint(&root.join("Scans").join(f)).unwrap()).collect();
            let projects = serde_json::json!({"version": 1, "projects": [{"id": oni_settings::Id::new().to_string(), "name": "Former", "description": "", "createdAt": 8e8, "members": prints}]});
            let favourites = serde_json::json!({"version": 1, "paths": [paths::canonical(&root.join("Scans"))]});
            std::fs::create_dir_all(root.join("Support")).unwrap();
            std::fs::write(root.join("Support/projects.json"), serde_json::to_vec(&projects).unwrap()).unwrap();
            std::fs::write(root.join("Support/favourites.json"), serde_json::to_vec(&favourites).unwrap()).unwrap();
        },
        due,
    );
    let support = hand.root.join("Support");
    let told = hand.shell.window.global::<Told>();
    assert!(told.get_shown() && told.get_message() == NOT_BROUGHT, "said at the launch: {}", told.get_message());
    told.invoke_closed();
    hand.rig.pump();

    // The old file's favourite stands; unstarred, the change is taken back and said.
    let (scans, starred) = starred_row(&hand, "Scans").expect("the old file's favourite");
    assert!(starred);
    hand.shell.window.global::<Navigator>().invoke_star_clicked(scans as i32);
    hand.rig.pump();
    assert!(told.get_shown() && told.get_message() == NOT_BROUGHT, "{}", told.get_message());
    assert_eq!(starred_row(&hand, "Scans").map(|(_, s)| s), Some(true), "still a favourite");

    // The old file's project holds its photos; a photo taken out comes back, said.
    let [a, b] = tones.map(|(f, _)| hand.at(f));
    hand.open("Roll");
    hand.lib().invoke_surface_picked(1);
    hand.rig.pump();
    assert_eq!(hand.shown(), [a.clone(), b.clone()], "each print found on its photo");
    hand.click(&b, false);
    hand.menu(&b, "Remove from Project", Some("Former"));
    assert_eq!(hand.shown(), [a.clone(), b.clone()], "taken back");
    assert!(told.get_shown() && told.get_message() == NOT_BROUGHT, "{}", told.get_message());
    assert!(!state::folder(&support).exists(), "nothing written under State/");

    // The next launch brings both, and the same gestures are kept.
    let mut hand = hand;
    hand.shell.window.hide().unwrap();
    assert_eq!(oni_app::migration::brought(&support, None), Brought::default());
    hand.shell = Hand::launch(&hand.root, Brought::default());
    assert_eq!(Places::load(&support).map(|p| p.paths()), Some(vec![paths::canonical(&hand.root.join("Scans"))]), "the favourite brought");
    hand.open("Roll");
    hand.lib().invoke_surface_picked(1);
    hand.rig.pump();
    assert_eq!(hand.shown(), [a.clone(), b.clone()], "the project brought");
    hand.click(&b, false);
    hand.menu(&b, "Remove from Project", Some("Former"));
    // Twin: with nothing due, the gesture lands and is written.
    assert_eq!(hand.shown(), [a]);
    assert_eq!(project::load(&support).projects[0].members.len(), 1);
}
