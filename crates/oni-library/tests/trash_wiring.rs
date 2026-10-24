//! `OniTrash.wiringCheck` read off the app's sources: Move to ONI Trash asks nothing and lays its
//! step, the opened ONI Trash selects into its own set and opens nothing, its rows restore unstarred.

use std::path::{Path, PathBuf};

fn source(relative: &str) -> String {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The block opened on the line holding `start`, up to the brace closing it at the same indent.
fn block(text: &str, start: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let Some(at) = lines.iter().position(|l| l.contains(start)) else { return Vec::new() };
    let indent = lines[at].len() - lines[at].trim_start().len();
    let closing = format!("{}}}", " ".repeat(indent));
    let end = lines[at + 1..].iter().position(|l| l.starts_with(&closing) && l.len() - l.trim_start().len() == indent);
    lines[at..at + 1 + end.unwrap_or(0)].iter().map(|l| l.to_string()).collect()
}

fn asks_first(body: &[String]) -> bool {
    body.iter().any(|l| l.contains(".ask(") || l.contains(".confirm(") || l.contains("Pending::"))
}

#[test]
fn move_to_oni_trash_lays_its_step_with_no_dialog_in_front_of_it() {
    let gestures = source("oni-app/src/app/gestures.rs");
    let busy = source("oni-app/src/app/gestures/busy.rs");
    let store = source("oni-core/src/store/oni_trash.rs");
    let moving: Vec<String> = [block(&gestures, "fn move_to_trash("), block(&gestures, "fn move_paths_to_trash("), block(&busy, "fn mark_off_thread(")]
        .concat();
    let landing = block(&busy, "fn marks_landed(");
    assert!(moving.len() > 6 && !asks_first(&moving), "the gesture asks nothing: {moving:#?}");
    assert!(moving.iter().any(|l| l.contains("mark_off_thread(")) && landing.iter().any(|l| l.contains("store.mark_landed(")));
    assert!(block(&store, "fn mark_landed(").iter().any(|l| l.contains("self.gallery.record_trash(")), "and lays one gallery step");
    // Twin: a confirmation in its body is named.
    let asking = [moving, vec!["        self.confirm(tr(\"Move\"), Pending::QuitAnyway);".into()]].concat();
    assert!(asks_first(&asking));
}

#[test]
fn the_opened_oni_trash_selects_into_its_own_set_and_a_tile_opens_nothing() {
    let organising = source("oni-app/src/app/organising.rs");
    let pointer = source("oni-app/src/app/organising/pointer.rs");
    let sealed = |picks: &[String], click: &[String]| {
        let picks = picks.join("\n");
        picks.contains("self.trash_shown()") && picks.contains("&self.trash_picks")
            && click.iter().any(|l| l.contains("Outcome::Open(path) if !trash =>"))
    };
    let (picks, click) = (block(&organising, "fn shown_picks(&self)"), block(&pointer, "fn click(self: &Rc<Self>"));
    assert!(sealed(&picks, &click), "{picks:#?}\n{click:#?}");
    // Twins: picks bound to the library's set, or a tile that opens in the ONI Trash, are named.
    let shared: Vec<String> = picks.iter().map(|l| l.replace("&self.trash_picks", "&self.picks")).collect();
    assert!(!sealed(&shared, &click));
    let opening: Vec<String> = click.iter().map(|l| l.replace(" if !trash =>", " =>")).collect();
    assert!(!sealed(&picks, &opening));
}

#[test]
fn a_row_of_the_oni_trash_restores_on_hover_and_is_never_starred() {
    let navigator = source("oni-ui/ui/navigator.slint");
    let row = source("oni-ui/ui/ds/row.slint");
    let unstarred = |lines: &[String]| {
        let text = lines.join("\n");
        text.contains("kind: RowKind.trash") && text.contains("@tr(\"RESTORE\")") && text.contains("acted => { root.trash-restore(i); }")
            && !text.contains("starrable") && !text.contains("favourite")
    };
    let trash = block(&navigator, "for line[i] in root.trash-rows: Row {");
    assert!(unstarred(&trash), "{trash:#?}");
    assert!(row.contains("if root.action != \"\" && root.hovered:"), "a row's action shows under the hover alone");
    // Twin: a star drawn on it is named.
    assert!(!unstarred(&[trash, vec!["                starrable: true;".into()]].concat()));
}
