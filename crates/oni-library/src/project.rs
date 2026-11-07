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
#[serde(from = "Written")]
pub struct Project {
    pub id: Id,
    pub name: String,
    pub description: String,
    #[serde(rename = "createdAt", with = "crate::reference_date")]
    pub created_at: Date,
    #[serde(rename = "manualOrder", skip_serializing_if = "Option::is_none", default)]
    pub manual_order: Option<f64>,
    /// Ordered paths, as two copies of one file in two rolls are two photos. A Swift file's are
    /// prints, each written as the path `project_standing` finds it at.
    pub members: Vec<String>,
}

/// The file as any app wrote it: the former spelling held prints, each with its last path.
#[derive(Deserialize)]
struct Written {
    id: Id,
    name: String,
    description: String,
    #[serde(rename = "createdAt", with = "crate::reference_date")]
    created_at: Date,
    #[serde(rename = "manualOrder", default)]
    manual_order: Option<f64>,
    members: Vec<String>,
    #[serde(default)]
    paths: HashMap<String, String>,
}

impl From<Written> for Project {
    fn from(written: Written) -> Project {
        let mut seen = HashSet::new();
        let members = written
            .members
            .into_iter()
            .map(|member| written.paths.get(&member).cloned().unwrap_or(member))
            .filter(|member| seen.insert(member.clone()))
            .collect();
        Project { id: written.id, name: written.name, description: written.description, created_at: written.created_at, manual_order: written.manual_order, members }
    }
}

impl Project {
    pub fn new(name: &str, created_at: Date, members: Vec<String>) -> Project {
        Project { id: Id::new(), name: name.to_owned(), description: String::new(), created_at, manual_order: None, members }
    }
}

/// A Swift file's member: a print, `size-date-digest`, which holds no slash where every path does.
pub fn is_print(member: &str) -> bool {
    !member.contains('/')
}

pub const UNTITLED_STEM: &str = "Project ";

pub fn untitled(taken: &[Project]) -> String {
    let numbers: HashSet<i64> = taken.iter().filter_map(|p| p.name.strip_prefix(UNTITLED_STEM).and_then(|n| n.parse().ok())).collect();
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

/// One project per member: the most recently created keeps it.
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

/// Every project, members resolved against the live library by path, newest first; a member no
/// row holds is skipped, never pruned.
pub fn grouped(projects: &[Project], entries: &[Entry]) -> Vec<ProjectGroup> {
    let mut by_path: HashMap<&str, &Entry> = HashMap::new();
    for entry in entries {
        by_path.entry(&entry.path).or_insert(entry);
    }
    let mut sorted: Vec<&Project> = projects.iter().collect();
    sorted.sort_by(|a, b| root_order(b).total_cmp(&root_order(a)));
    sorted
        .into_iter()
        .map(|project| ProjectGroup {
            id: project.id,
            project: project.clone(),
            entries: project.members.iter().filter_map(|m| by_path.get(m.as_str()).map(|e| (*e).clone())).collect(),
        })
        .collect()
}

/// Each project's members by id, what an Undo step holds at either end of a gesture on them.
pub fn rosters(projects: &[Project]) -> BTreeMap<Id, Vec<String>> {
    projects.iter().map(|p| (p.id, p.members.clone())).collect()
}

/// The members a step held laid back on each project still there; one deleted since stays deleted.
pub fn regroup(projects: &mut [Project], rosters: &BTreeMap<Id, Vec<String>>) {
    for project in projects.iter_mut() {
        if let Some(members) = rosters.get(&project.id) {
            project.members = members.clone();
        }
    }
}

/// A folder or a file moved in ONI carries the members under it, each move tried in order.
pub fn moved(projects: &[Project], moves: &[(String, String)]) -> Vec<Project> {
    let carried = |path: &str| moves.iter().find(|(from, _)| holds(from, path)).map_or_else(|| path.to_owned(), |(from, to)| crate::paths::moved(path, from, to));
    projects
        .iter()
        .map(|project| Project { members: project.members.iter().map(|m| carried(m)).collect(), ..project.clone() })
        .collect()
}

// MARK: - Gestures

/// `newProject(from:)`: the photos picked, in library order, under the lowest free name, none making
/// an empty one; it leaves no other project, the file settling exclusivity when read again.
pub fn new_from(projects: &mut Vec<Project>, entries: &[Entry], paths: &HashSet<String>, now: Date) -> Id {
    let members: Vec<String> = entries.iter().filter(|e| paths.contains(&e.path)).map(|e| e.path.clone()).collect();
    let made = Project::new(&untitled(projects), now, members);
    let id = made.id;
    projects.push(made);
    id
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
pub fn leave(projects: &mut [Project], members: &HashSet<String>, kept: Id) {
    for project in projects.iter_mut().filter(|p| p.id != kept) {
        project.members.retain(|m| !members.contains(m));
    }
}

/// The projects `add_to` would take photos from, each with how many: what the ask names first.
pub fn leaving<'a>(projects: &'a [Project], paths: &HashSet<String>, id: Id) -> Vec<(&'a Project, usize)> {
    projects
        .iter()
        .filter(|p| p.id != id)
        .map(|p| (p, p.members.iter().filter(|m| paths.contains(*m)).count()))
        .filter(|(_, n)| *n > 0)
        .collect()
}

/// What Add to Project did: the photos that joined, and those already members, left as they were.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Joined {
    pub added: usize,
    pub already: usize,
}

/// `addToProject`: the photos not already members appended in library order, each leaving the
/// project it sat in.
pub fn add_to(projects: &mut [Project], entries: &[Entry], paths: &HashSet<String>, id: Id) -> Joined {
    let Some(at) = projects.iter().position(|p| p.id == id) else { return Joined::default() };
    let picked: Vec<String> = entries.iter().filter(|e| paths.contains(&e.path)).map(|e| e.path.clone()).collect();
    let (already, joining): (Vec<String>, Vec<String>) = picked.into_iter().partition(|p| projects[at].members.contains(p));
    let done = Joined { added: joining.len(), already: already.len() };
    if !joining.is_empty() {
        leave(projects, &joining.iter().cloned().collect(), id);
        projects[at].members.extend(joining);
    }
    done
}

/// `removeFromProject`: the photos leave it, how many said; a project left empty stays, named, until deleted.
pub fn remove_from(projects: &mut [Project], paths: &HashSet<String>, id: Id) -> usize {
    let Some(project) = projects.iter_mut().find(|p| p.id == id) else { return 0 };
    let before = project.members.len();
    project.members.retain(|m| !paths.contains(m));
    before - project.members.len()
}

/// The projects holding any of `paths`, in the file's order.
pub fn member_projects<'a>(projects: &'a [Project], paths: &HashSet<String>) -> Vec<&'a Project> {
    projects.iter().filter(|p| p.members.iter().any(|m| paths.contains(m))).collect()
}

/// `reorderProjectDrop`: the dragged photos land before `target` in the target's project, those
/// already members first in their own order, the rest joining from other projects.
pub fn reorder_drop(projects: &mut [Project], entries: &[Entry], dragged: &HashSet<String>, target: &str) -> bool {
    let Some(at) = projects.iter().position(|p| p.members.iter().any(|m| m == target)) else { return false };
    let mut moving: HashSet<String> = entries.iter().filter(|e| dragged.contains(&e.path)).map(|e| e.path.clone()).collect();
    moving.remove(target);
    if moving.is_empty() {
        return false;
    }
    let id = projects[at].id;
    leave(projects, &moving, id);
    let mut members = projects[at].members.clone();
    let already: Vec<String> = members.iter().filter(|m| moving.contains(*m)).cloned().collect();
    let joining: Vec<String> =
        entries.iter().filter(|e| moving.contains(&e.path) && !already.contains(&e.path)).map(|e| e.path.clone()).collect();
    members.retain(|m| !already.contains(m));
    let Some(insert) = members.iter().position(|m| m == target) else { return false };
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
    crate::replace_readable(&file_path(support), text.as_bytes())
}

#[cfg(test)]
mod tests;
