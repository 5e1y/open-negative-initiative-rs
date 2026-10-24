//! Three-way merges of the support files the Swift app writes too: what this app changed since it
//! read the file is laid over what the file holds now, so another app's change is never lost.

use crate::project::{self, Project, ProjectsFile};
use oni_settings::Id;
use std::collections::BTreeMap;
use std::path::Path;

/// `ours` against `base`, laid over `theirs`: what this side removed goes, what it added comes
/// last, in its order; either side unchanged gives the other whole, its order included.
pub fn list(base: &[String], ours: &[String], theirs: &[String]) -> Vec<String> {
    if theirs == base {
        return ours.to_vec();
    }
    if ours == base {
        return theirs.to_vec();
    }
    let removed = |p: &String| base.contains(p) && !ours.contains(p);
    let mut out: Vec<String> = theirs.iter().filter(|p| !removed(p)).cloned().collect();
    for added in ours.iter().filter(|p| !base.contains(p)) {
        if !out.contains(added) {
            out.push(added.clone());
        }
    }
    out
}

/// This side's value where it changed it, else the file's.
fn pick<T: PartialEq + Clone>(base: &T, ours: &T, theirs: &T) -> T {
    if ours != base { ours.clone() } else { theirs.clone() }
}

/// Projects by id: one this side deleted stays deleted, one the other deleted stays deleted unless
/// this side changed it since; each field merged on its own, so a member added elsewhere survives.
pub fn projects(base: &[Project], ours: &[Project], theirs: &[Project]) -> Vec<Project> {
    let find = |list: &[Project], id: Id| list.iter().find(|p| p.id == id).cloned();
    let mut out = Vec::new();
    for mine in ours {
        match (find(base, mine.id), find(theirs, mine.id)) {
            (None, _) => out.push(mine.clone()),
            (Some(read), None) if !edited(&read, mine) => {}
            (Some(_), None) => out.push(mine.clone()),
            (Some(read), Some(other)) => out.push(project(&read, mine, &other)),
        }
    }
    for other in theirs {
        let known = base.iter().any(|p| p.id == other.id);
        if !known && !out.iter().any(|p| p.id == other.id) {
            out.push(other.clone());
        }
    }
    out
}

/// Whether this side changed what a hand sees; the paths a save writes down are not a change.
fn edited(read: &Project, mine: &Project) -> bool {
    Project { paths: None, ..read.clone() } != Project { paths: None, ..mine.clone() }
}

fn project(base: &Project, ours: &Project, theirs: &Project) -> Project {
    let members = list(&base.members, &ours.members, &theirs.members);
    let mut paths: BTreeMap<String, String> = theirs.paths.clone().unwrap_or_default();
    let read = base.paths.clone().unwrap_or_default();
    for (member, path) in ours.paths.iter().flatten() {
        if read.get(member) != Some(path) {
            paths.insert(member.clone(), path.clone());
        } else {
            paths.entry(member.clone()).or_insert_with(|| path.clone());
        }
    }
    paths.retain(|member, _| members.contains(member));
    Project {
        id: ours.id,
        name: pick(&base.name, &ours.name, &theirs.name),
        description: pick(&base.description, &ours.description, &theirs.description),
        created_at: pick(&base.created_at, &ours.created_at, &theirs.created_at),
        manual_order: pick(&base.manual_order, &ours.manual_order, &theirs.manual_order),
        members,
        paths: (!paths.is_empty()).then_some(paths),
    }
}

/// `projects.json` read again and merged with what this side changed since `base`, then written;
/// the list written comes back, the next save's base. A file absent or unreadable takes ours whole.
pub fn save_projects(support: &Path, base: &[Project], ours: &[Project]) -> std::io::Result<Vec<Project>> {
    let theirs = crate::read_json::<ProjectsFile>(&project::file_path(support));
    let merged = match theirs {
        Some(file) => project::exclusive(&projects(base, ours, &project::exclusive(&file.projects))),
        None => ours.to_vec(),
    };
    project::save(support, &ProjectsFile { version: 1, projects: merged.clone() })?;
    Ok(merged)
}

#[cfg(test)]
mod tests;
