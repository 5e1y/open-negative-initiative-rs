//! A curated cross-roll selection laid over the library's order. Membership lives in
//! `projects.json`, not in a sidecar: an ordered member list cannot fit one sidecar field.

use crate::index::Entry;
use crate::paths::holds;
use oni_settings::json;
use oni_settings::{Date, Id};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: Id,
    pub name: String,
    pub description: String,
    #[serde(rename = "createdAt", with = "crate::reference_date")]
    pub created_at: Date,
    #[serde(rename = "manualOrder", skip_serializing_if = "Option::is_none", default)]
    pub manual_order: Option<f64>,
    /// Ordered fingerprints, not paths: a path changes on relink, a fingerprint does not.
    pub members: Vec<String>,
    /// Each member's last known path, by fingerprint, read first: keeps a photo no read has reached.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub paths: Option<BTreeMap<String, String>>,
}

impl Project {
    pub fn new(name: &str, created_at: Date, members: Vec<String>) -> Project {
        Project {
            id: Id::new(),
            name: name.to_owned(),
            description: String::new(),
            created_at,
            manual_order: None,
            members,
            paths: None,
        }
    }
}

pub const UNTITLED_STEM: &str = "Project ";

pub fn untitled(taken: &[Project]) -> String {
    let numbers: std::collections::HashSet<i64> =
        taken.iter().filter_map(|p| p.name.strip_prefix(UNTITLED_STEM).and_then(|n| n.parse().ok())).collect();
    let mut number = 1;
    while numbers.contains(&number) {
        number += 1;
    }
    format!("{UNTITLED_STEM}{number}")
}

/// A project's place among its root siblings: hand-placed if dragged, else its creation date.
pub fn root_order(project: &Project) -> f64 {
    project.manual_order.unwrap_or_else(|| project.created_at.time_interval_since_reference_date())
}

/// One project per fingerprint: the most recently created keeps it.
pub fn exclusive(projects: &[Project]) -> Vec<Project> {
    let mut keeper: HashMap<&str, Id> = HashMap::new();
    let mut newest: HashMap<&str, Date> = HashMap::new();
    for project in projects {
        for member in &project.members {
            if newest.get(member.as_str()).copied().unwrap_or_else(crate::distant_past) < project.created_at {
                keeper.insert(member, project.id);
                newest.insert(member, project.created_at);
            }
        }
    }
    projects
        .iter()
        .map(|project| {
            let mut kept = project.clone();
            kept.members.retain(|m| keeper.get(m.as_str()) == Some(&project.id));
            kept
        })
        .collect()
}

/// A project's titles are always the name a hand gave it.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectGroup {
    pub id: Id,
    pub project: Project,
    pub entries: Vec<Entry>,
}

impl ProjectGroup {
    pub fn display_title(&self) -> &str {
        &self.project.name
    }

    pub fn explicit_name(&self) -> Option<&str> {
        Some(&self.project.name)
    }
}

/// Every project, members resolved against the live library by path then fingerprint, newest
/// first; a dangling fingerprint is skipped, never pruned.
pub fn grouped(projects: &[Project], entries: &[Entry]) -> Vec<ProjectGroup> {
    let mut by_fingerprint: HashMap<&str, &Entry> = HashMap::new();
    let mut by_path: HashMap<&str, &Entry> = HashMap::new();
    for entry in entries {
        by_fingerprint.entry(&entry.fingerprint).or_insert(entry);
        by_path.entry(&entry.path).or_insert(entry);
    }
    let mut sorted: Vec<&Project> = projects.iter().collect();
    sorted.sort_by(|a, b| root_order(b).total_cmp(&root_order(a)));
    sorted
        .into_iter()
        .map(|project| ProjectGroup {
            id: project.id,
            project: project.clone(),
            entries: project
                .members
                .iter()
                .filter_map(|member| {
                    project
                        .paths
                        .as_ref()
                        .and_then(|p| p.get(member))
                        .and_then(|path| by_path.get(path.as_str()))
                        .or_else(|| by_fingerprint.get(member.as_str()))
                        .map(|e| (*e).clone())
                })
                .collect(),
        })
        .collect()
}

/// Each project with every resolved member's path written down, a member no row resolves
/// keeping the path it had.
pub fn located(projects: &[Project], entries: &[Entry]) -> Vec<Project> {
    let groups: HashMap<Id, ProjectGroup> = grouped(projects, entries).into_iter().map(|g| (g.id, g)).collect();
    projects
        .iter()
        .map(|project| {
            let mut kept = project.clone();
            let mut resolved: HashMap<&str, &str> = HashMap::new();
            if let Some(group) = groups.get(&project.id) {
                for e in &group.entries {
                    resolved.entry(&e.fingerprint).or_insert(&e.path);
                }
            }
            let mut paths = BTreeMap::new();
            for member in &project.members {
                let path = resolved
                    .get(member.as_str())
                    .map(|p| p.to_string())
                    .or_else(|| project.paths.as_ref().and_then(|p| p.get(member)).cloned());
                if let Some(path) = path {
                    paths.insert(member.clone(), path);
                }
            }
            kept.paths = (!paths.is_empty()).then_some(paths);
            kept
        })
        .collect()
}

/// A folder moved in ONI carries its members' paths, each move tried in order.
pub fn moved(projects: &[Project], moves: &[(String, String)]) -> Vec<Project> {
    projects
        .iter()
        .map(|project| {
            let mut kept = project.clone();
            if let Some(paths) = kept.paths.as_mut() {
                for path in paths.values_mut() {
                    if let Some((from, to)) = moves.iter().find(|(from, _)| holds(from, path)) {
                        *path = crate::paths::moved(path, from, to);
                    }
                }
            }
            kept
        })
        .collect()
}

// MARK: - Gestures

/// `newProject(from:)`: the photos picked, in library order, under the lowest free name. Joining
/// leaves no other project: exclusivity is settled when the file is read again.
pub fn new_from(projects: &mut Vec<Project>, entries: &[Entry], paths: &HashSet<String>, now: Date) -> Option<Id> {
    let fingerprints: Vec<String> = entries.iter().filter(|e| paths.contains(&e.path)).map(|e| e.fingerprint.clone()).collect();
    if fingerprints.is_empty() {
        return None;
    }
    let made = Project::new(&untitled(projects), now, fingerprints);
    let id = made.id;
    projects.push(made);
    Some(id)
}

pub fn rename(projects: &mut [Project], id: Id, name: &str) -> bool {
    let Some(project) = projects.iter_mut().find(|p| p.id == id) else { return false };
    project.name = name.to_owned();
    true
}

pub fn set_description(projects: &mut [Project], id: Id, text: &str) -> bool {
    let Some(project) = projects.iter_mut().find(|p| p.id == id) else { return false };
    project.description = text.to_owned();
    true
}

pub fn delete(projects: &mut Vec<Project>, id: Id) {
    projects.retain(|p| p.id != id);
}

/// A photo sits in one project, so joining `kept` leaves every other. A project left empty stays,
/// its name and description kept: only Delete Project, which asks, takes one away.
pub fn leave(projects: &mut [Project], fingerprints: &HashSet<String>, kept: Id) {
    for project in projects.iter_mut().filter(|p| p.id != kept) {
        project.members.retain(|m| !fingerprints.contains(m));
    }
}

/// The projects `add_to` would take photos from, each with how many: what the ask names first.
pub fn leaving<'a>(projects: &'a [Project], entries: &[Entry], paths: &HashSet<String>, id: Id) -> Vec<(&'a Project, usize)> {
    let fingerprints: HashSet<&str> = entries.iter().filter(|e| paths.contains(&e.path)).map(|e| e.fingerprint.as_str()).collect();
    projects
        .iter()
        .filter(|p| p.id != id)
        .map(|p| (p, p.members.iter().filter(|m| fingerprints.contains(m.as_str())).count()))
        .filter(|(_, n)| *n > 0)
        .collect()
}

/// `addToProject`: by fingerprint, the photos not already members appended; false when none joins.
pub fn add_to(projects: &mut [Project], entries: &[Entry], paths: &HashSet<String>, id: Id) -> bool {
    let Some(at) = projects.iter().position(|p| p.id == id) else { return false };
    let mut joining: Vec<String> = Vec::new();
    for e in entries.iter().filter(|e| paths.contains(&e.path)) {
        if !projects[at].members.contains(&e.fingerprint) {
            joining.push(e.fingerprint.clone());
        }
    }
    if joining.is_empty() {
        return false;
    }
    leave(projects, &joining.iter().cloned().collect(), id);
    let at = projects.iter().position(|p| p.id == id).expect("the kept project stays");
    projects[at].members.extend(joining);
    true
}

/// `removeFromProject`: the photos leave it; a project left empty stays, named, until deleted.
pub fn remove_from(projects: &mut [Project], entries: &[Entry], paths: &HashSet<String>, id: Id) {
    let Some(at) = projects.iter().position(|p| p.id == id) else { return };
    let fingerprints: HashSet<&str> = entries.iter().filter(|e| paths.contains(&e.path)).map(|e| e.fingerprint.as_str()).collect();
    projects[at].members.retain(|m| !fingerprints.contains(m.as_str()));
}

/// The projects holding any of `paths`, keyed on fingerprints, in the file's order.
pub fn member_projects<'a>(projects: &'a [Project], entries: &[Entry], paths: &HashSet<String>) -> Vec<&'a Project> {
    let fingerprints: HashSet<&str> = entries.iter().filter(|e| paths.contains(&e.path)).map(|e| e.fingerprint.as_str()).collect();
    projects.iter().filter(|p| p.members.iter().any(|m| fingerprints.contains(m.as_str()))).collect()
}

/// `reorderProjectDrop`: the dragged photos land before `target` in the target's project, those
/// already members first in their own order, the rest joining from other projects.
pub fn reorder_drop(projects: &mut [Project], entries: &[Entry], dragged: &HashSet<String>, target: &Entry) -> bool {
    let Some(at) = projects.iter().position(|p| p.members.contains(&target.fingerprint)) else { return false };
    let mut moving: HashSet<String> =
        entries.iter().filter(|e| dragged.contains(&e.path)).map(|e| e.fingerprint.clone()).collect();
    moving.remove(&target.fingerprint);
    if moving.is_empty() {
        return false;
    }
    let id = projects[at].id;
    leave(projects, &moving, id);
    let at = projects.iter().position(|p| p.id == id).expect("the kept project stays");
    let mut members = projects[at].members.clone();
    let already: Vec<String> = members.iter().filter(|m| moving.contains(*m)).cloned().collect();
    let joining: Vec<String> = entries
        .iter()
        .filter(|e| moving.contains(&e.fingerprint) && !already.contains(&e.fingerprint))
        .map(|e| e.fingerprint.clone())
        .collect();
    members.retain(|m| !already.contains(m));
    let Some(insert) = members.iter().position(|m| *m == target.fingerprint) else { return false };
    members.splice(insert..insert, already.into_iter().chain(joining));
    projects[at].members = members;
    true
}

/// `reorderProjectsRoot`: projects sort newest first, so the dragged ones take orders stepping
/// down from the target's towards the sibling above it, or a day above when it heads the list.
pub fn reorder_root(projects: &mut [Project], entries: &[Entry], dragged: &HashSet<Id>, target: Id) -> bool {
    let siblings: Vec<Id> = grouped(projects, entries).into_iter().map(|g| g.project.id).collect();
    let Some(target_at) = siblings.iter().position(|id| *id == target) else { return false };
    let movable: Vec<Id> = siblings.iter().filter(|id| **id != target && dragged.contains(id)).copied().collect();
    if movable.is_empty() {
        return false;
    }
    let order = |id: Id| projects.iter().find(|p| p.id == id).map(root_order).unwrap_or(0.0);
    let ahead = order(target);
    let behind = if target_at > 0 { order(siblings[target_at - 1]) } else { ahead + 86_400.0 };
    let step = (ahead - behind) / (movable.len() + 1) as f64;
    for (i, id) in movable.iter().enumerate() {
        if let Some(project) = projects.iter_mut().find(|p| p.id == *id) {
            project.manual_order = Some(ahead - step * (i + 1) as f64);
        }
    }
    true
}

/// `deleteProjectRequested`: the confirmation's title, message and key.
pub fn delete_prompt(project: &Project) -> (String, String, &'static str) {
    let count = project.members.len();
    let message = if count > 0 {
        oni_i18n::trn(
            "It stops grouping its {n} photo. The photos, their settings and every other project or roll they belong to are untouched.",
            "It stops grouping its {n} photos. The photos, their settings and every other project or roll they belong to are untouched.",
            count as u64,
        )
    } else {
        oni_i18n::tr("It holds no photos. Nothing else is affected.").to_owned()
    };
    (oni_i18n::tr!("Delete \u{201c}{name}\u{201d}?", name = project.name), message, oni_i18n::tr("Delete Project"))
}

/// The one file every project lives in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectsFile {
    pub version: i64,
    pub projects: Vec<Project>,
}

impl Default for ProjectsFile {
    fn default() -> Self {
        ProjectsFile { version: 1, projects: Vec::new() }
    }
}

pub fn file_path(support: &Path) -> PathBuf {
    support.join("projects.json")
}

pub fn load(support: &Path) -> ProjectsFile {
    let Some(mut file) = crate::read_json::<ProjectsFile>(&file_path(support)) else {
        return ProjectsFile::default();
    };
    file.projects = exclusive(&file.projects);
    file
}

/// The text `JSONEncoder` writes with `.prettyPrinted` and `.sortedKeys`.
pub fn encode(file: &ProjectsFile) -> Result<String, json::Error> {
    Ok(json::pretty_sorted(&json::to_node(file)?))
}

pub fn save(support: &Path, file: &ProjectsFile) -> std::io::Result<()> {
    std::fs::create_dir_all(support)?;
    let text = encode(file).map_err(std::io::Error::other)?;
    crate::write_atomically(&file_path(support), text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    fn stamp() -> Date {
        Date::since_reference_date(8e8)
    }

    fn row(path: &str, fingerprint: &str) -> Entry {
        let mut e = Entry::new(path, stamp(), fingerprint);
        e.captured_at = Some(stamp());
        e
    }

    fn members(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn identity_is_the_id_and_the_free_name_the_lowest_number() {
        let a = Project::new("Iceland", stamp(), vec![]);
        let mut b = a.clone();
        b.description = String::new();
        assert_eq!(a, b);
        assert_ne!(a, Project::new("Iceland", stamp(), vec![]));
        let taken = [Project::new("Project 1", stamp(), vec![]), Project::new("Project 3", stamp(), vec![])];
        assert_eq!(untitled(&taken), "Project 2");
        assert_eq!(untitled(&[]), "Project 1");
    }

    #[test]
    fn a_file_round_trips_through_the_disk_in_the_swift_spelling() {
        let scratch = Scratch::new("projects");
        let mut iceland = Project::new("Iceland", stamp(), members(&["fp-a", "fp-b"]));
        iceland.description = "Two weeks, June".into();
        let mut portraits = Project::new("Portraits", stamp(), vec![]);
        portraits.manual_order = Some(3.0);
        let file = ProjectsFile { version: 1, projects: vec![iceland, portraits] };
        save(scratch.path(), &file).unwrap();
        assert_eq!(load(scratch.path()), file);
        let text = std::fs::read_to_string(file_path(scratch.path())).unwrap();
        assert!(text.contains("\"createdAt\" : 800000000"), "{text}");
        assert!(text.contains("\"manualOrder\" : 3"));
        // Twin: the sidecar's ISO spelling of the same date is not what a project writes.
        assert!(!text.contains(&stamp().iso8601()));
    }

    #[test]
    fn projects_order_newest_first_and_resolve_by_path_then_fingerprint() {
        let older = Project::new("Older", Date::since_reference_date(0.0), vec![]);
        let newer = Project::new("Newer", Date::since_reference_date(100.0), vec![]);
        let mut pinned = Project::new("Pinned", Date::since_reference_date(0.0), vec![]);
        pinned.manual_order = Some(200.0);
        let order: Vec<String> = grouped(&[older, newer, pinned], &[]).into_iter().map(|g| g.project.name).collect();
        assert_eq!(order, ["Pinned", "Newer", "Older"]);

        let library = [row("/negs/a.RW2", "fp-a"), row("/negs/b.RW2", "fp-b")];
        let gap = Project::new("Gap", stamp(), members(&["fp-b", "fp-missing", "fp-a"]));
        let resolved: Vec<String> =
            grouped(std::slice::from_ref(&gap), &library)[0].entries.iter().map(|e| e.path.clone()).collect();
        assert_eq!(resolved, ["/negs/b.RW2", "/negs/a.RW2"]);
        assert!(gap.members.contains(&"fp-missing".to_owned()), "a dangling member is never pruned by reading");

        let mut trip = Project::new("Trip", stamp(), members(&["fp-1", "fp-2"]));
        trip.paths = Some(BTreeMap::from([("fp-1".to_owned(), "/r/a.RW2".to_owned())]));
        let by_path = grouped(std::slice::from_ref(&trip), &[row("/r/a.RW2", "fp-other"), row("/s/a.RW2", "fp-1")]);
        assert_eq!(by_path[0].entries[0].path, "/r/a.RW2");
        let relinked = grouped(std::slice::from_ref(&trip), &[row("/s/a.RW2", "fp-1")]);
        assert_eq!(relinked[0].entries[0].path, "/s/a.RW2");
        let stamped = located(std::slice::from_ref(&trip), &[row("/r/a.RW2", "fp-1"), row("/r/b.RW2", "fp-2")]);
        assert_eq!(stamped[0].paths.as_ref().map(|p| p.len()), Some(2));
        // Twin: a member no row resolves keeps the path it had.
        let kept = located(std::slice::from_ref(&trip), &[]);
        assert_eq!(kept[0].paths, trip.paths);
        let carried = moved(std::slice::from_ref(&trip), &[("/r".into(), "/t/r".into())]);
        assert_eq!(carried[0].paths.as_ref().unwrap()["fp-1"], "/t/r/a.RW2");
        let sibling = moved(std::slice::from_ref(&trip), &[("/r/a".into(), "/x".into())]);
        assert_eq!(
            sibling[0].paths.as_ref().unwrap()["fp-1"],
            "/r/a.RW2",
            "a prefix that is not a folder moves nothing"
        );
    }

    #[test]
    fn a_shared_fingerprint_stays_with_the_newest_project() {
        let first = Project::new("First", Date::since_reference_date(0.0), members(&["fp-a", "fp-shared"]));
        let second = Project::new("Second", Date::since_reference_date(100.0), members(&["fp-shared", "fp-b"]));
        let settled: Vec<Vec<String>> =
            exclusive(&[first.clone(), second.clone()]).into_iter().map(|p| p.members).collect();
        assert_eq!(settled, [members(&["fp-a"]), members(&["fp-shared", "fp-b"])]);
        // Twin: listed the other way round, creation still decides, not position.
        let swapped: Vec<Vec<String>> = exclusive(&[second, first]).into_iter().map(|p| p.members).collect();
        assert_eq!(swapped, [members(&["fp-shared", "fp-b"]), members(&["fp-a"])]);
    }

    fn picked(paths: &[&str]) -> HashSet<String> {
        paths.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_new_project_takes_the_picked_photos_in_library_order_under_the_lowest_free_name() {
        let library = [row("/r/a", "fa"), row("/r/b", "fb"), row("/r/c", "fc")];
        let mut projects = vec![Project::new("Project 1", stamp(), members(&["fz"]))];
        let id = new_from(&mut projects, &library, &picked(&["/r/c", "/r/a"]), stamp()).unwrap();
        let made = projects.iter().find(|p| p.id == id).unwrap().clone();
        assert_eq!((made.name.as_str(), made.members.clone()), ("Project 2", members(&["fa", "fc"])));
        assert!(new_from(&mut projects, &library, &picked(&["/elsewhere"]), stamp()).is_none(), "nothing picked, nothing made");
        // Twin: the picked set's own order would put c first.
        assert_ne!(made.members, members(&["fc", "fa"]));
    }

    /// "Add to Project" takes a photo out of the project it sat in, as a photo sits in one: the ask
    /// names that project first, and an emptied project stays, never dropped without a word.
    #[test]
    fn adding_to_a_project_never_drops_another_silently() {
        let library = [row("/r/a", "fa"), row("/r/b", "fb"), row("/r/c", "fc")];
        let first = Project::new("First", stamp(), members(&["fa"]));
        let second = Project::new("Second", stamp(), members(&["fb", "fc"]));
        let target = Project::new("Target", stamp(), members(&["fz"]));
        let (first_id, second_id, target_id) = (first.id, second.id, target.id);
        let mut projects = vec![first, second, target];
        let asked: Vec<(&str, usize)> = leaving(&projects, &library, &picked(&["/r/a", "/r/b"]), target_id).into_iter().map(|(p, n)| (p.name.as_str(), n)).collect();
        assert_eq!(asked, [("First", 1), ("Second", 1)], "the ask names each project a photo leaves");
        assert!(leaving(&projects, &library, &picked(&["/r/c"]), second_id).is_empty(), "twin: photos already there leave nothing");
        assert!(add_to(&mut projects, &library, &picked(&["/r/a", "/r/b"]), target_id));
        let first = projects.iter().find(|p| p.id == first_id).expect("First stays, empty, with its name");
        assert!(first.members.is_empty() && first.name == "First");
        assert_eq!(projects.iter().find(|p| p.id == second_id).unwrap().members, members(&["fc"]));
        assert_eq!(projects.iter().find(|p| p.id == target_id).unwrap().members, members(&["fz", "fa", "fb"]));
        // Twin: the rule it replaces, emptied projects dropped, loses First and its name.
        let mut dropped = projects.clone();
        dropped.retain(|p| !p.members.is_empty());
        assert!(!dropped.iter().any(|p| p.id == first_id));
        // Adding members already there changes nothing and says so.
        let before = projects.clone();
        assert!(!add_to(&mut projects, &library, &picked(&["/r/a"]), target_id));
        assert_eq!(projects, before);
    }

    #[test]
    fn a_drop_lands_before_its_target_members_first_then_the_joining_ones() {
        let library = [row("/r/a", "fa"), row("/r/b", "fb"), row("/r/c", "fc"), row("/r/d", "fd"), row("/r/x", "fx")];
        let trip = Project::new("Trip", stamp(), members(&["fa", "fb", "fc", "fd"]));
        let other = Project::new("Other", stamp(), members(&["fx"]));
        let trip_id = trip.id;
        let mut projects = vec![other, trip];
        // d and x dragged before b: d already a member moves first, x joins after it and leaves Other.
        assert!(reorder_drop(&mut projects, &library, &picked(&["/r/x", "/r/d"]), &library[1]));
        assert!(projects[0].name == "Other" && projects[0].members.is_empty(), "Other held only x and stays, empty");
        assert_eq!(projects[1].id, trip_id);
        assert_eq!(projects[1].members, members(&["fa", "fd", "fx", "fb", "fc"]));
        // Twin: the target dragged onto itself moves nothing.
        assert!(!reorder_drop(&mut projects, &library, &picked(&["/r/b"]), &library[1]));
        // A drop on a photo in no project lands nowhere.
        let loose = row("/r/loose", "floose");
        assert!(!reorder_drop(&mut projects, &library, &picked(&["/r/a"]), &loose));
    }

    #[test]
    fn a_project_dragged_among_its_siblings_takes_an_order_between_the_target_and_the_one_above() {
        let old = Project::new("Old", Date::since_reference_date(0.0), vec![]);
        let mid = Project::new("Mid", Date::since_reference_date(100.0), vec![]);
        let new = Project::new("New", Date::since_reference_date(200.0), vec![]);
        let (old_id, mid_id, new_id) = (old.id, mid.id, new.id);
        let mut projects = vec![old, mid, new];
        let order = |p: &[Project]| -> Vec<String> { grouped(p, &[]).into_iter().map(|g| g.project.name).collect() };
        assert_eq!(order(&projects), ["New", "Mid", "Old"]);
        assert!(reorder_root(&mut projects, &[], &HashSet::from([old_id]), mid_id));
        assert_eq!(order(&projects), ["New", "Old", "Mid"], "dropped before Mid, it shows above it");
        assert_eq!(projects[0].manual_order, Some(150.0));
        // Before the head, a day above it frames the step.
        assert!(reorder_root(&mut projects, &[], &HashSet::from([mid_id]), new_id));
        assert_eq!(order(&projects), ["Mid", "New", "Old"]);
        assert_eq!(projects[1].manual_order, Some(200.0 + 86_400.0 / 2.0));
        // Twin: the target alone dragged onto itself is refused.
        assert!(!reorder_root(&mut projects, &[], &HashSet::from([new_id]), new_id));
    }
}
