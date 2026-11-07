//! A 1.5.6 file naming a photo by its placeholder in one project and by its print in another, as the
//! Projects tab shows it: once, at the print, a project holding only the placeholder counting nothing.

use oni_app::Confirm;
use oni_library::{library::WAITING_MARK, project};
use slint::{ComponentHandle, Model};

use crate::projects_tab::Hand;

/// The project rows as the column lists them, newest first: title, caption, whether it opens.
fn rows(hand: &Hand) -> Vec<(String, String, bool)> {
    hand.lib().get_project_rows().iter().map(|r| (r.title.to_string(), r.caption.to_string(), r.expandable)).collect()
}

/// The delete prompt's message for the project row `i`, the prompt then answered `yes`.
fn asked(hand: &Hand, i: i32, yes: bool) -> String {
    let lib = hand.lib();
    assert!(lib.invoke_project_row_menu(i));
    let lines: Vec<String> = lib.get_menu_lines().iter().map(|l| l.text.to_string()).collect();
    lib.invoke_menu_chosen(lines.iter().position(|l| l == "Delete Project").expect("Delete Project in the menu") as i32);
    hand.rig.pump();
    let confirm = hand.shell.window.global::<Confirm>();
    assert!(confirm.get_shown());
    let message = confirm.get_message().to_string();
    confirm.invoke_answered(yes);
    hand.rig.pump();
    message
}

/// Portfolio holds A by its print and B by a print B no longer carries; Trip names A by 1.5.6's
/// placeholder, Tour names B so: A shows in Portfolio alone, B in Tour, Portfolio's B a missing tile.
pub(crate) fn a_placeholder_hidden_by_a_print_counts_nothing_and_its_prompt_says_so() {
    let tones = [("Roll/A.tif", 0.3), ("Roll/B.tif", 0.6)];
    let hand = Hand::made("placeholder", &tones, |root| {
        let at = |f: &str| oni_library::paths::canonical(&root.join("Scans").join(f));
        let (a, b) = (at("Roll/A.tif"), at("Roll/B.tif"));
        let print = oni_library::relink::fingerprint(std::path::Path::new(&a)).unwrap();
        let (wa, wb) = (format!("{WAITING_MARK}{a}"), format!("{WAITING_MARK}{b}"));
        let made = |name: &str, at: f64, texts: &[(&str, &str)]| {
            let paths: serde_json::Map<String, serde_json::Value> = texts.iter().map(|(t, p)| (t.to_string(), (*p).into())).collect();
            let members: Vec<&str> = texts.iter().map(|(t, _)| *t).collect();
            serde_json::json!({"id": oni_settings::Id::new().to_string(), "name": name, "description": "", "createdAt": at, "members": members, "paths": paths})
        };
        let projects = [made("Portfolio", 8e8, &[(&print, &a), ("1-stale", &b)]), made("Trip", 9e8, &[(&wa, &a)]), made("Tour", 9.5e8, &[(&wb, &b)])];
        std::fs::create_dir_all(root.join("Support")).unwrap();
        std::fs::write(project::file_path(&root.join("Support")), serde_json::to_vec(&serde_json::json!({"version": 1, "projects": projects})).unwrap()).unwrap();
    });
    let support = hand.root.join("Support");
    let original = std::fs::read(project::file_path(&support)).unwrap();
    hand.open("Roll");
    hand.lib().invoke_surface_picked(1);
    hand.rig.pump();
    let counted = |name: &str, caption: &str, opens: bool| (name.to_string(), caption.to_string(), opens);
    assert_eq!(rows(&hand), [counted("Tour", "1", true), counted("Trip", "0", false), counted("Portfolio", "2", true)], "A once, in Portfolio");
    assert_eq!(std::fs::read(project::file_path(&support)).unwrap(), original, "the read learns nothing and writes nothing");
    let trip = asked(&hand, 1, true);
    assert_eq!(trip, "It shows no photo: the one it names stands in another project, which keeps it. Nothing else is affected.");
    let names: Vec<String> = project::load(&support).projects.into_iter().map(|p| p.name).collect();
    assert_eq!(names, ["Portfolio", "Tour"], "Trip deleted, A still Portfolio's");
    // Twin: Tour's placeholder, its file carrying no print a member holds, shows B and counts it.
    assert!(asked(&hand, 0, false).starts_with("It stops grouping its 1 photo."));
}
