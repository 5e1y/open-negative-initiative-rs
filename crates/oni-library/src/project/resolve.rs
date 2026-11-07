//! Each member matched to the row it stands for: at its path while the row there carries its print,
//! else where the last read of the index found it by that print. Reading never rewrites a member.

use super::member::Member;
use super::{Project, root_order};
use crate::index::Entry;
use std::collections::{BTreeMap, HashMap, HashSet};

/// Where a read found each member its own path no longer shows: the path of the row it stands for.
pub type Found = BTreeMap<Member, String>;

static NONE: Found = BTreeMap::new();

/// What members resolve against: the rows reached, and where the last read found members.
#[derive(Debug, Clone, Copy)]
pub struct Reach<'a> {
    pub rows: &'a [Entry],
    pub found: &'a Found,
}

impl<'a> Reach<'a> {
    pub fn new(rows: &'a [Entry], found: &'a Found) -> Reach<'a> {
        Reach { rows, found }
    }

    /// The rows alone, each member found at its own path or nowhere.
    pub fn rows(rows: &'a [Entry]) -> Reach<'a> {
        Reach { rows, found: &NONE }
    }
}

/// Each project's members, in order, with the row each stands for; a row a member stands at by its
/// own path is never also where another was found.
pub fn resolve<'a>(projects: &[Project], reach: &Reach<'a>) -> Vec<Vec<Option<&'a Entry>>> {
    let mut by_path: HashMap<&str, &'a Entry> = HashMap::new();
    for row in reach.rows {
        by_path.entry(row.path.as_str()).or_insert(row);
    }
    let mut claimed: HashSet<&str> = HashSet::new();
    let mut out: Vec<Vec<Option<&'a Entry>>> = projects.iter().map(|p| vec![None; p.members.len()]).collect();
    for (p, project) in projects.iter().enumerate() {
        for (m, member) in project.members.iter().enumerate() {
            let Some(row) = member.path.as_deref().and_then(|path| by_path.get(path)) else { continue };
            if !reach.found.contains_key(member) && member.agrees(&row.fingerprint) {
                claimed.insert(row.path.as_str());
                out[p][m] = Some(*row);
            }
        }
    }
    for (p, project) in projects.iter().enumerate() {
        for (m, member) in project.members.iter().enumerate() {
            let Some(row) = reach.found.get(member).and_then(|path| by_path.get(path.as_str())) else { continue };
            if claimed.insert(row.path.as_str()) {
                out[p][m] = Some(*row);
            }
        }
    }
    out
}

/// The member each row shown stands for, by the row's path: its project's place and its own.
pub fn holders(projects: &[Project], reach: &Reach) -> HashMap<String, (usize, usize)> {
    let mut out = HashMap::new();
    for (p, rows) in resolve(projects, reach).into_iter().enumerate() {
        for (m, row) in rows.into_iter().enumerate() {
            if let Some(row) = row {
                out.insert(row.path.clone(), (p, m));
            }
        }
    }
    out
}

/// A project's titles are always the name a hand gave it.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectGroup {
    pub id: oni_settings::Id,
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

/// Every project with the rows its members stand for, newest first; a member no row shows is
/// skipped, never pruned.
pub fn grouped(projects: &[Project], reach: &Reach) -> Vec<ProjectGroup> {
    let rows = resolve(projects, reach);
    let mut groups: Vec<ProjectGroup> = projects
        .iter()
        .zip(rows)
        .map(|(project, rows)| ProjectGroup { id: project.id, project: project.clone(), entries: rows.into_iter().flatten().cloned().collect() })
        .collect();
    groups.sort_by(|a, b| root_order(&b.project).total_cmp(&root_order(&a.project)));
    groups
}
