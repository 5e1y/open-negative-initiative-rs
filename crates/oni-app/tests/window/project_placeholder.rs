//! A 1.5.6 file naming a photo by its placeholder in one project and by its print in another, as the
//! Projects tab shows it once read: once, at the print, a project holding only the placeholder counting nothing.

use oni_app::{Confirm, Told};
use oni_library::{library::WAITING_MARK, project};
use slint::{ComponentHandle, Model};

use crate::projects_tab::Hand;

/// The project rows as the column lists them, newest first: title, caption, whether it opens.
fn rows(hand: &Hand) -> Vec<(String, String, bool)> {
    hand.lib().get_project_rows().iter().filter(|r| r.depth == 0).map(|r| (r.title.to_string(), r.caption.to_string(), r.expandable)).collect()
}

/// The delete prompt's message for the project titled `name`, the prompt then answered `yes`.
fn asked(hand: &Hand, name: &str, yes: bool) -> String {
    let lib = hand.lib();
    let i = lib.get_project_rows().iter().position(|r| r.depth == 0 && r.title.as_str() == name).expect("the project's row");
    assert!(lib.invoke_project_row_menu(i as i32));
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

/// Remove from Project on `path`'s tile out of `name`: what the notice says, the step then undone.
fn removed(hand: &Hand, path: &str, name: &str) -> String {
    hand.menu(path, "Remove from Project", Some(name));
    let said = hand.told().expect("a notice").1;
    hand.shell.window.global::<Told>().invoke_acted();
    hand.rig.pump();
    said
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
        // The file as an earlier build wrote it, imported as a launch does.
        std::fs::write(root.join("Support/projects.json"), serde_json::to_vec(&serde_json::json!({"version": 1, "projects": projects})).unwrap()).unwrap();
        oni_app::migration::brought(&root.join("Support"), None);
    });
    let support = hand.root.join("Support");
    let (a, b) = (hand.at("Roll/A.tif"), hand.at("Roll/B.tif"));
    hand.open("Roll");
    hand.lib().invoke_surface_picked(1);
    hand.rig.pump();
    let counted = |name: &str, caption: &str, opens: bool| (name.to_string(), caption.to_string(), opens);
    assert_eq!(rows(&hand), [counted("Tour", "1", true), counted("Trip", "0", false), counted("Portfolio", "2", true)], "A once, in Portfolio");
    let held = |name: &str| project::load(&support).projects.into_iter().find(|p| p.name == name).unwrap().members;
    let print_b = oni_library::relink::fingerprint(std::path::Path::new(&b)).unwrap();
    assert_eq!(held("Tour"), [project::Member::new(Some(&b), Some(&print_b))], "alone at B once read, Tour's placeholder is written as B's print");
    assert_eq!(held("Trip"), [project::Member::new(Some(&a), Some(&format!("{WAITING_MARK}{a}")))], "Trip's, beside Portfolio's print, as given");
    // Removed from Portfolio, A shows in Trip by its placeholder, and the notice says so.
    assert_eq!(removed(&hand, &a, "Portfolio"), "One photo removed from \u{201c}Portfolio\u{201d}. It now shows in \u{201c}Trip\u{201d}.");
    // Twin: B shows in no other project once out of Tour, Portfolio's stale print a missing tile elsewhere.
    assert_eq!(removed(&hand, &b, "Tour"), "One photo removed from \u{201c}Tour\u{201d}.");
    assert_eq!(rows(&hand), [counted("Tour", "1", true), counted("Trip", "0", false), counted("Portfolio", "2", true)], "both undone");
    let trip = asked(&hand, "Trip", true);
    assert_eq!(trip, "It shows no photo: the one it names stands in another project, which keeps it. Nothing else is affected.");
    let names: Vec<String> = project::load(&support).projects.into_iter().map(|p| p.name).collect();
    assert_eq!(names, ["Portfolio", "Tour"], "Trip deleted, A still Portfolio's");
    // Twin: Tour's placeholder, its file carrying no print a member holds, shows B and counts it.
    assert!(asked(&hand, "Tour", false).starts_with("It stops grouping its 1 photo."));
}
