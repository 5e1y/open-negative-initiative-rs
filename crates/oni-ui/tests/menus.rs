//! `EditorMenus.selfCheck`: two menu items claiming one keystroke leave the second gesture dead, as
//! the system gives the key to the first item found and greys nothing.

use std::collections::BTreeMap;

/// Every declared shortcut, its chord with the spaces taken out, and how often it is claimed.
fn claims(slint: &str) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for rest in slint.split("shortcut: @keys(").skip(1) {
        let chord: String = rest.split(')').next().unwrap_or_default().chars().filter(|c| !c.is_whitespace()).collect();
        *out.entry(chord).or_insert(0) += 1;
    }
    out
}

fn clashes(slint: &str) -> Vec<String> {
    claims(slint).into_iter().filter(|(_, n)| *n > 1).map(|(c, _)| c).collect()
}

#[test]
fn every_menu_shortcut_is_claimed_once() {
    let app = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/ui/app.slint")).unwrap();
    assert!(claims(&app).len() > 20, "the menus' shortcuts are read: {:?}", claims(&app));
    assert!(clashes(&app).is_empty(), "claimed twice: {:?}", clashes(&app));
    // Twin: the roll's ⌘⇧G written as the mode toggle's plain ⌘G is caught.
    let collided = app.replace("@keys(Control + Shift + \"g\")", "@keys(Control + \"g\")");
    assert_ne!(collided, app);
    assert_eq!(clashes(&collided), ["Control+\"g\""]);
}

/// The menu lines whose shortcut is a bare key, and whether each answers only while the window's
/// own key scope holds the focus.
fn bare_keys(slint: &str) -> Vec<(String, bool)> {
    slint
        .lines()
        .filter_map(|l| {
            let chord = l.split("shortcut: @keys(").nth(1)?.split(')').next()?;
            let bare = !["Control", "Alt", "Meta"].iter().any(|m| chord.contains(m));
            bare.then(|| (chord.to_string(), l.contains("&& keys.has-focus;")))
        })
        .collect()
}

/// `DSTextFocus.selfCheck`, as the source states it: a focused text field keeps its bare keys, so
/// no bare-key menu line answers while the focus is elsewhere than the window's key scope.
#[test]
fn a_bare_key_answers_only_while_the_window_holds_the_focus() {
    let app = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/ui/app.slint")).unwrap();
    let keys = bare_keys(&app);
    assert!(keys.len() >= 6 && keys.iter().all(|(_, guarded)| *guarded), "{keys:?}");
    // Twin: a medal line written without the guard is caught.
    let open = app.replace("enabled: Menus.live.medal && keys.has-focus; activated => { App.menu(Act.medal-silver); }", "enabled: Menus.live.medal; activated => { App.menu(Act.medal-silver); }");
    assert_ne!(open, app);
    assert!(bare_keys(&open).iter().any(|(chord, guarded)| chord == "\"2\"" && !guarded));
}
