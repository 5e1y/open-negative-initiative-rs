//! Each member matched to the row it stands for: at its path while the row there carries its print,
//! else where the last read of the index found it by that print. Reading never rewrites a member.
//! A placeholder stands at the file at its path where no other member does, shows it beside one there
//! until the file is read, and is hidden behind it once read: worked out at each read.

use super::member::{Member, read};
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
/// own path is never also where another was found, nor where a placeholder stands once read.
pub fn resolve<'a>(projects: &[Project], reach: &Reach<'a>) -> Vec<Vec<Option<&'a Entry>>> {
    stands(projects, reach).into_iter().map(|rows| rows.into_iter().map(|r| r.map(|(row, _)| row)).collect()).collect()
}

/// `resolve`, each row marked whether its member holds it: a placeholder in another project shows a
/// file not read yet beside its holder, as nothing proves the holder's print there yet.
pub(super) fn stands<'a>(projects: &[Project], reach: &Reach<'a>) -> Vec<Vec<Option<(&'a Entry, bool)>>> {
    let mut by_path: HashMap<&str, &'a Entry> = HashMap::new();
    for row in reach.rows {
        by_path.entry(row.path.as_str()).or_insert(row);
    }
    let mut claimed: HashSet<&str> = HashSet::new();
    let mut out: Vec<Vec<Option<(&'a Entry, bool)>>> = projects.iter().map(|p| vec![None; p.members.len()]).collect();
    let placed = || projects.iter().enumerate().flat_map(|(p, project)| project.members.iter().enumerate().map(move |(m, member)| (p, m, member)));
    for (p, m, member) in placed().filter(|(_, _, member)| member.placeholder().is_none()) {
        let Some(row) = member.path.as_deref().and_then(|path| by_path.get(path)) else { continue };
        if !reach.found.contains_key(member) && member.agrees(&row.fingerprint) {
            claimed.insert(row.path.as_str());
            out[p][m] = Some((*row, true));
        }
    }
    for (p, m, member) in placed().filter(|(_, _, member)| member.placeholder().is_none()) {
        let Some(row) = reach.found.get(member).and_then(|path| by_path.get(path.as_str())) else { continue };
        if claimed.insert(row.path.as_str()) {
            out[p][m] = Some((*row, true));
        }
    }
    // The newest project's placeholder first, as the newest keeps a photo.
    let mut waiting: Vec<(usize, usize, &Member)> = placed().filter(|(_, _, member)| member.placeholder().is_some()).collect();
    let made = |p: usize| projects[p].created_at.time_interval_since_reference_date();
    waiting.sort_by(|a, b| made(b.0).total_cmp(&made(a.0)));
    for (p, m, member) in waiting {
        let Some(row) = place(member, reach).and_then(|path| by_path.get(path)) else { continue };
        let shown = |out: &[Option<(&Entry, bool)>]| out.iter().flatten().any(|(r, _)| r.path == row.path);
        if claimed.insert(row.path.as_str()) {
            out[p][m] = Some((*row, true));
        } else if read(&row.fingerprint).is_none() && !shown(&out[p]) {
            out[p][m] = Some((*row, false));
        }
    }
    out
}

/// Where a placeholder stands: the row the read found at its path, else its path.
pub(super) fn place<'m>(member: &'m Member, reach: &'m Reach) -> Option<&'m str> {
    reach.found.get(member).map(String::as_str).or(member.path.as_deref())
}

/// Each project's placeholders another member stands in for, the file at their path shown at that member alone.
pub fn shadowed(projects: &[Project], reach: &Reach) -> Vec<usize> {
    let rows: HashSet<&str> = reach.rows.iter().map(|row| row.path.as_str()).collect();
    let hidden = |member: &Member, row: &Option<&Entry>| row.is_none() && member.placeholder().is_some() && place(member, reach).is_some_and(|path| rows.contains(path));
    projects.iter().zip(resolve(projects, reach)).map(|(project, rows)| project.members.iter().zip(&rows).filter(|(m, row)| hidden(m, row)).count()).collect()
}

/// The member each row shown stands for, by the row's path: its project's place and its own.
pub fn holders(projects: &[Project], reach: &Reach) -> HashMap<String, (usize, usize)> {
    let mut out = HashMap::new();
    for (p, rows) in stands(projects, reach).into_iter().enumerate() {
        for (m, row) in rows.into_iter().enumerate() {
            if let Some((row, true)) = row {
                out.insert(row.path.clone(), (p, m));
            }
        }
    }
    out
}

/// Every member each row shown stands for or beside, by the row's path, its holder first.
pub(super) fn showing(projects: &[Project], reach: &Reach) -> HashMap<String, Vec<(usize, usize)>> {
    let mut out: HashMap<String, Vec<(usize, usize)>> = HashMap::new();
    for (p, rows) in stands(projects, reach).into_iter().enumerate() {
        for (m, (row, holds)) in rows.into_iter().enumerate().filter_map(|(m, row)| Some((m, row?))) {
            let at = out.entry(row.path.clone()).or_default();
            if holds { at.insert(0, (p, m)) } else { at.push((p, m)) }
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
