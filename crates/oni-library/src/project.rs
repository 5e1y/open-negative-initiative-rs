//! A curated cross-roll selection laid over the library's order. Membership lives in
//! `projects.json`, not in a sidecar: an ordered member list cannot fit one sidecar field.

use crate::index::Entry;
use crate::paths::holds;
use oni_settings::json;
use oni_settings::{Date, Id};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
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
                    for (from, to) in moves {
                        if path == from {
                            *path = to.clone();
                            break;
                        }
                        if holds(from, path) {
                            *path = format!("{to}{}", &path[from.len()..]);
                            break;
                        }
                    }
                }
            }
            kept
        })
        .collect()
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
    let Some(mut file) =
        std::fs::read(file_path(support)).ok().and_then(|data| serde_json::from_slice::<ProjectsFile>(&data).ok())
    else {
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
}
