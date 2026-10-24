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
    Paste,
    Reset,
    ResetToDefault,
    Medal(oni_settings::Medal),
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

/// `Medal.label`.
pub fn medal_name(medal: oni_settings::Medal) -> &'static str {
    use oni_settings::Medal;
    match medal {
        Medal::None => tr("None"),
        Medal::Bronze => tr("Bronze"),
        Medal::Silver => tr("Silver"),
        Medal::Gold => tr("Gold"),
    }
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

/// `CellMenu.body`, line by line; a submenu's opener is a line whose action is `AddTo` or `RemoveFrom`.
pub fn cell_menu(scope: &HashSet<String>, order: &[String], facts: &CellFacts) -> Vec<CellLine> {
    use oni_settings::Medal;
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
        line(counted(n, "Paste Settings", tr!("Paste Settings ({count} photos)", count = n)), facts.can_paste && any, gone > 0, CellAction::Paste, &here, gone),
        line(counted(n, "Reset Settings", tr!("Reset Settings ({count} photos)", count = n)), any, false, CellAction::Reset, &here, gone),
        line(counted(n, "Reset to Default…", tr!("Reset to Default… ({count} photos)", count = n)), any, false, CellAction::ResetToDefault, &here, gone),
    ]);
    for (i, medal) in [Medal::None, Medal::Bronze, Medal::Silver, Medal::Gold].into_iter().enumerate() {
        lines.push(line(medal_name(medal).into(), any, i == 0, CellAction::Medal(medal), &here, gone));
    }
    lines.push(line(counted(n, "Move into a New Roll…", tr!("Move into a New Roll… ({count} photos)", count = n)), any, true, CellAction::NewRoll, &here, gone));
    let j = joined.len();
    lines.push(line(counted(j, "Ungroup", tr!("Ungroup ({count} photos)", count = j)), !joined.is_empty(), false, CellAction::Ungroup, &joined, gone));
    let all = scope.len();
    lines.push(line(counted(all, "New Project from", tr!("New Project from ({count} photos)", count = all)), true, false, CellAction::NewProject, scope, 0));
    if !facts.projects.is_empty() {
        lines.push(line(tr("Add to Project").into(), true, false, CellAction::AddTo, scope, 0));
    }
    if !facts.member_of.is_empty() {
        lines.push(line(tr("Remove from Project").into(), true, false, CellAction::RemoveFrom, scope, 0));
    }
    lines.push(line(counted(n, "Export…", tr!("Export… ({count} photos)", count = n)), any, true, CellAction::Export, &here, 0));
    let reveal = if n > 1 { platform::reveal_photos(Platform::THIS, n) } else { platform::reveal(Platform::THIS).to_owned() };
    lines.push(line(reveal, any, false, CellAction::Reveal, &here, 0));
    let trash = if n > 1 { tr!("Move {count} Photos to ONI Trash", count = n) } else { tr("Move to ONI Trash").into() };
    lines.push(line(trash, any, true, CellAction::Trash, &here, gone));
    lines
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
                "1 photo missing", "Paste Settings", "Reset Settings", "Reset to Default\u{2026}",
                "None", "Bronze", "Silver", "Gold",
                "Move into a New Roll\u{2026}", "Ungroup", "New Project from (2 photos)", "Add to Project",
                "Export\u{2026}", platform::reveal(Platform::THIS), "Move to ONI Trash"
            ]
        );
        assert!(!lines[0].enabled && lines[0].action == CellAction::Missing, "the count first, greyed");
        assert!(!lines[1].enabled, "Paste waits for copied settings");
        assert_eq!(lines[9].scope, paths(&["a"]), "Ungroup reaches only the photos in a roll");
        // Each verb that writes or moves acts on the photo there and says the one left out.
        for at in [2, 3, 5, 8, 14] {
            assert_eq!((lines[at].scope.clone(), lines[at].left, lines[at].enabled), (paths(&["a"]), 1, true), "{}", lines[at].text);
        }
        assert_eq!(lines[10].scope, paths(&["a", "b"]), "a project names a photo away too");
        assert_eq!((lines[12].scope.clone(), lines[12].left), (paths(&["a"]), 0), "a missing original is not exported");
        let rules: Vec<usize> = lines.iter().enumerate().filter(|(_, l)| l.rule).map(|(i, _)| i).collect();
        assert_eq!(rules, [1, 4, 8, 12, 14]);
        let sub = submenu(&lines[11], &facts);
        assert_eq!((sub[0].text.as_str(), sub[0].action.clone()), ("Trip", CellAction::AddToProject(trip)));
        // Twins: a scope all away writes nothing; one all there has no count and leaves none out.
        let away = cell_menu(&set(&["b"]), &order, &facts);
        assert!(away.iter().filter(|l| l.left > 0).all(|l| !l.enabled), "every write greyed on photos all away");
        assert!(away.iter().any(|l| l.action == CellAction::NewProject && l.enabled));
        let there = cell_menu(&set(&["a", "c"]), &order, &facts);
        assert_eq!(there[0].text, "Paste Settings (2 photos)");
        assert!(there.iter().all(|l| l.left == 0) && there.iter().all(|l| l.action != CellAction::Missing));
        // Twin: no project at all offers no Add to Project, and a scope in none no Remove.
        let bare = CellFacts { projects: vec![], ..facts };
        assert!(!cell_menu(&set(&["c"]), &order, &bare).iter().any(|l| matches!(l.action, CellAction::AddTo | CellAction::RemoveFrom)));
        assert_eq!(trash_menu(&set(&["a"]), &order)[0].0, "Restore");
        assert_eq!(trash_menu(&selection, &order)[0].0, "Restore 2 Photos");
    }
}
