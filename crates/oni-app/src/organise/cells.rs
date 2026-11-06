//! The cell menu's lines, `CellMenu`: each verb with its scope; a photo whose original is out of
//! reach is counted first, and every verb that writes beside an original or moves it leaves it out.

use crate::platform::{self, Platform};
use oni_i18n::{tr, trn};
use std::collections::HashSet;

/// What a line of the cell menu does, `CellMenu`'s buttons in its order.
#[derive(Clone, Debug, PartialEq)]
pub enum CellAction {
    /// The greyed count of the photos out of reach, which does nothing.
    Missing,
    Copy,
    Paste,
    Reset,
    NewRoll,
    Ungroup,
    NewProject,
    /// Opens the projects to add to; the chosen one comes back as `AddToProject`.
    AddTo,
    AddToProject(oni_settings::Id),
    RemoveFrom,
    RemoveFromProject(oni_settings::Id),
    Export,
    Reveal,
    Trash,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CellLine {
    pub text: String,
    pub enabled: bool,
    pub rule: bool,
    pub action: CellAction,
    /// The photos it acts on, which also become the selection.
    pub scope: Vec<String>,
    /// The photos of the menu's scope it leaves out, their originals out of reach, said once it acts.
    pub left: usize,
}

/// `CellMenu.targets`: the selection when the pointed photo is in it, else the photo alone.
pub fn cell_scope(path: &str, selection: &HashSet<String>) -> HashSet<String> {
    if selection.contains(path) { selection.clone() } else { HashSet::from([path.to_owned()]) }
}

/// Where the menu's facts come from, read only when the menu opens.
pub struct CellFacts<'a> {
    pub missing: &'a HashSet<String>,
    pub rolled: &'a HashSet<String>,
    pub can_paste: bool,
    /// Every project, then those holding any photo of the scope, as (id, name).
    pub projects: Vec<(oni_settings::Id, String)>,
    pub member_of: Vec<(oni_settings::Id, String)>,
}

fn sorted(set: &HashSet<String>, order: &[String]) -> Vec<String> {
    let mut v: Vec<String> = order.iter().filter(|p| set.contains(*p)).cloned().collect();
    let mut rest: Vec<String> = set.iter().filter(|p| !v.contains(p)).cloned().collect();
    rest.sort();
    v.extend(rest);
    v
}

impl CellAction {
    /// The chord the menu bar answers for the same gesture, as `platform` writes it; none where it has none.
    pub fn chord(&self, platform: Platform) -> String {
        let k = platform::keys(platform);
        match self {
            CellAction::Copy => format!("{}C", k.command_shift),
            CellAction::Paste => format!("{}V", k.command_shift),
            CellAction::Reset => format!("{}R", k.command_shift),
            CellAction::NewRoll => format!("{}G", k.command_shift),
            CellAction::Ungroup => format!("{}G", k.option_command_shift),
            CellAction::Export => format!("{}E", k.command),
            CellAction::Trash => format!("{}{}", k.command, k.delete),
            _ => String::new(),
        }
    }
}

/// `CellMenu.body`, a submenu's opener being a line of `AddTo` or `RemoveFrom`; the medals and the
/// roll's reset stay with the status bar and the Rolls menu, as this menu acts on its photos alone.
/// What resets or removes stands last, in a section of its own, apart from every verb that keeps.
pub fn cell_menu(scope: &HashSet<String>, order: &[String], facts: &CellFacts) -> Vec<CellLine> {
    let here: HashSet<String> = scope.difference(facts.missing).cloned().collect();
    let gone = scope.len() - here.len();
    let joined: HashSet<String> = here.intersection(facts.rolled).cloned().collect();
    // A verb that writes a sidecar or moves a file acts on the photos there and counts the rest.
    let line = |text: String, enabled: bool, rule: bool, action: CellAction, on: &HashSet<String>, left: usize| CellLine { text, enabled, rule, action, scope: sorted(on, order), left };
    let (n, any) = (here.len(), !here.is_empty());
    // `menu_label`'s words: the verb alone for one photo, counted past it.
    let counted = |count: usize, one: &'static str, many: String| if count > 1 { many } else { tr(one).to_owned() };
    let mut lines = Vec::new();
    if gone > 0 {
        let missing: HashSet<String> = scope.intersection(facts.missing).cloned().collect();
        lines.push(line(trn("{n} photo missing", "{n} photos missing", gone as u64), false, false, CellAction::Missing, &missing, 0));
    }
    lines.extend([
        // One photo's settings at a time, as the editor's Copy takes the open frame's.
        line(tr("Copy Settings\u{2026}").into(), n == 1 && gone == 0, gone > 0, CellAction::Copy, &here, 0),
        line(counted(n, "Paste Settings", tr!("Paste Settings ({count} photos)", count = n)), facts.can_paste && any, false, CellAction::Paste, &here, gone),
    ]);
    lines.push(line(counted(n, "Move into a New Roll…", tr!("Move into a New Roll… ({count} photos)", count = n)), any, true, CellAction::NewRoll, &here, gone));
    let j = joined.len();
    lines.push(line(counted(j, "Ungroup", tr!("Ungroup ({count} photos)", count = j)), !joined.is_empty(), false, CellAction::Ungroup, &joined, gone));
    let all = scope.len();
    lines.push(line(new_project(all), true, false, CellAction::NewProject, scope, 0));
    if !facts.projects.is_empty() {
        lines.push(line(tr("Add to Project").into(), true, false, CellAction::AddTo, scope, 0));
    }
    if !facts.member_of.is_empty() {
        lines.push(line(tr("Remove from Project").into(), true, false, CellAction::RemoveFrom, scope, 0));
    }
    lines.push(line(counted(n, "Export…", tr!("Export… ({count} photos)", count = n)), any, true, CellAction::Export, &here, 0));
    let reveal = if n > 1 { platform::reveal_photos(Platform::THIS, n) } else { platform::reveal(Platform::THIS).to_owned() };
    lines.push(line(reveal, any, false, CellAction::Reveal, &here, 0));
    lines.push(line(counted(n, "Reset Settings", tr!("Reset Settings ({count} photos)", count = n)), any, true, CellAction::Reset, &here, gone));
    let trash = if n > 1 { tr!("Move {count} Photos to ONI Trash", count = n) } else { tr("Move to ONI Trash").into() };
    lines.push(line(trash, any, false, CellAction::Trash, &here, gone));
    lines
}

/// New Project's words, in the cell menu and the menu bar alike: the photos it is made from.
pub fn new_project(count: usize) -> String {
    if count > 1 { tr!("New Project from {count} Photos", count = count) } else { tr("New Project from This Photo").into() }
}

/// The submenu a line opens: the projects to add to, or those to leave.
pub fn submenu(opener: &CellLine, facts: &CellFacts) -> Vec<CellLine> {
    let (list, make): (&[(oni_settings::Id, String)], fn(oni_settings::Id) -> CellAction) = match opener.action {
        CellAction::AddTo => (&facts.projects, CellAction::AddToProject),
        CellAction::RemoveFrom => (&facts.member_of, CellAction::RemoveFromProject),
        _ => return Vec::new(),
    };
    list.iter().map(|(id, name)| CellLine { text: name.clone(), enabled: true, rule: false, action: make(*id), scope: opener.scope.clone(), left: 0 }).collect()
}

/// The ONI Trash's cell menu: Restore, then Show in Finder or Explorer.
pub fn trash_menu(scope: &HashSet<String>, order: &[String]) -> Vec<(String, bool, Vec<String>)> {
    let n = scope.len();
    let on = sorted(scope, order);
    vec![(if n > 1 { tr!("Restore {count} Photos", count = n) } else { tr("Restore").into() }, false, on.clone()), (platform::reveal(Platform::THIS).into(), true, on)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn set(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn the_cell_menu_acts_on_the_selection_it_points_into_and_counts_each_verbs_own_scope() {
        let order = paths(&["a", "b", "c"]);
        let selection = set(&["a", "b"]);
        assert_eq!(cell_scope("a", &selection), selection);
        assert_eq!(cell_scope("c", &selection), set(&["c"]), "a photo outside the selection is its own scope");
        let (missing, rolled) = (set(&["b"]), set(&["a"]));
        let trip = oni_settings::Id::new();
        let facts = CellFacts { missing: &missing, rolled: &rolled, can_paste: false, projects: vec![(trip, "Trip".into())], member_of: vec![] };
        let lines = cell_menu(&selection, &order, &facts);
        let texts: Vec<&str> = lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "1 photo missing", "Copy Settings\u{2026}", "Paste Settings", "Move into a New Roll\u{2026}", "Ungroup",
                "New Project from 2 Photos", "Add to Project", "Export\u{2026}", platform::reveal(Platform::THIS),
                "Reset Settings", "Move to ONI Trash"
            ]
        );
        assert!(!lines[0].enabled && lines[0].action == CellAction::Missing, "the count first, greyed");
        assert!(!lines[1].enabled, "Copy takes one photo whole, never one with a photo away beside it");
        assert!(!lines[2].enabled, "Paste waits for copied settings");
        assert_eq!(lines[4].scope, paths(&["a"]), "Ungroup reaches only the photos in a roll");
        // Each verb that writes or moves acts on the photo there and says the one left out.
        for at in [3, 9, 10] {
            assert_eq!((lines[at].scope.clone(), lines[at].left, lines[at].enabled), (paths(&["a"]), 1, true), "{}", lines[at].text);
        }
        assert_eq!(lines[5].scope, paths(&["a", "b"]), "a project names a photo away too");
        assert_eq!((lines[7].scope.clone(), lines[7].left), (paths(&["a"]), 0), "a missing original is not exported");
        // Reset and the ONI Trash share the foot's section, ruled off from every verb that keeps.
        let rules: Vec<usize> = lines.iter().enumerate().filter(|(_, l)| l.rule).map(|(i, _)| i).collect();
        assert_eq!(rules, [1, 3, 7, 9]);
        let sub = submenu(&lines[6], &facts);
        assert_eq!((sub[0].text.as_str(), sub[0].action.clone()), ("Trip", CellAction::AddToProject(trip)));
        // Twins: a scope all away writes nothing; one all there has no count and leaves none out.
        let away = cell_menu(&set(&["b"]), &order, &facts);
        assert!(away.iter().filter(|l| l.left > 0).all(|l| !l.enabled), "every write greyed on photos all away");
        assert!(away.iter().any(|l| l.action == CellAction::NewProject && l.enabled));
        let there = cell_menu(&set(&["a", "c"]), &order, &facts);
        assert_eq!((there[1].text.as_str(), there[0].enabled), ("Paste Settings (2 photos)", false), "Copy greyed on two photos");
        assert_eq!(there[4].text, "New Project from 2 Photos");
        assert!(there.iter().all(|l| l.left == 0) && there.iter().all(|l| l.action != CellAction::Missing));
        // Twin: one photo there copies its settings, and New Project names it alone.
        let one = cell_menu(&set(&["c"]), &order, &facts);
        assert_eq!((one[0].action.clone(), one[0].enabled, one[0].scope.clone()), (CellAction::Copy, true, paths(&["c"])));
        assert_eq!(one[4].text, "New Project from This Photo");
        // Twin: no project at all offers no Add to Project, and a scope in none no Remove.
        let bare = CellFacts { projects: vec![], ..facts };
        assert!(!cell_menu(&set(&["c"]), &order, &bare).iter().any(|l| matches!(l.action, CellAction::AddTo | CellAction::RemoveFrom)));
        assert_eq!(trash_menu(&set(&["a"]), &order)[0].0, "Restore");
        assert_eq!(trash_menu(&selection, &order)[0].0, "Restore 2 Photos");
    }

    /// A menu bar line's chord, read off `app.slint`, as the Mac writes it: the item that calls `act`.
    fn declared(app: &str, act: &str) -> Option<String> {
        let line = app.lines().find(|l| l.contains(&format!("App.menu({act});")))?;
        let chord = line.split("@keys(").nth(1)?.split(')').next()?;
        let parts: Vec<&str> = chord.split('+').map(str::trim).collect();
        let k = platform::keys(Platform::Mac);
        let key = parts.last()?.trim_matches('"').to_uppercase();
        let key = if key == "BACKSPACE" { k.delete.to_owned() } else { key };
        let lead = match (parts.contains(&"Alt"), parts.contains(&"Shift")) {
            (true, true) => k.option_command_shift,
            (true, false) => k.option_command,
            (false, true) => k.command_shift,
            (false, false) => k.command,
        };
        Some(format!("{lead}{key}"))
    }

    #[test]
    fn each_line_prints_the_chord_the_menu_bar_answers_for_its_gesture() {
        let app = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../oni-ui/ui/app.slint")).unwrap();
        let pairs = [
            (CellAction::Copy, "Act.copy"),
            (CellAction::Paste, "Act.paste"),
            (CellAction::Reset, "Act.reset"),
            (CellAction::NewRoll, "Act.new-roll"),
            (CellAction::Ungroup, "Act.ungroup"),
            (CellAction::Export, "Act.export"),
            (CellAction::Trash, "Act.remove"),
        ];
        for (action, act) in &pairs {
            assert_eq!(Some(action.chord(Platform::Mac)), declared(&app, act), "{action:?}");
        }
        assert_eq!((CellAction::Trash.chord(Platform::Mac), CellAction::Ungroup.chord(Platform::Windows)), ("⌘⌫".into(), "Ctrl+Alt+Shift+G".into()));
        assert!(CellAction::Reveal.chord(Platform::Mac).is_empty() && CellAction::NewProject.chord(Platform::Mac).is_empty());
        // Twin: the menu bar's Reset moved to another chord is caught.
        let moved = app.replace("@keys(Control + Shift + \"r\")", "@keys(Control + \"r\")");
        assert_ne!(moved, app);
        assert_ne!(Some(CellAction::Reset.chord(Platform::Mac)), declared(&moved, "Act.reset"));
    }
}
