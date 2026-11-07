//! Three-way merges of the support files the Swift app writes too: what this app changed since it
//! read the file is laid over what the file holds now, so another app's change is never lost.

use crate::project::{self, Project, ProjectsFile};
use oni_settings::Id;
use std::path::Path;

/// `ours` against `base`, laid over `theirs`: what this side removed goes, what it added comes
/// last, in its order; either side unchanged gives the other whole, its order included.
pub fn list<T: PartialEq + Clone>(base: &[T], ours: &[T], theirs: &[T]) -> Vec<T> {
    if theirs == base {
        return ours.to_vec();
    }
    if ours == base {
        return theirs.to_vec();
    }
    let removed = |p: &T| base.contains(p) && !ours.contains(p);
    let mut out: Vec<T> = theirs.iter().filter(|p| !removed(p)).cloned().collect();
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
            (Some(read), None) if read == *mine => {}
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

fn project(base: &Project, ours: &Project, theirs: &Project) -> Project {
    Project {
        id: ours.id,
        name: pick(&base.name, &ours.name, &theirs.name),
        description: pick(&base.description, &ours.description, &theirs.description),
        created_at: pick(&base.created_at, &ours.created_at, &theirs.created_at),
        manual_order: pick(&base.manual_order, &ours.manual_order, &theirs.manual_order),
        members: project::distinct(list(&base.members, &ours.members, &theirs.members)),
    }
}

/// `projects.json` read again, merged with what this side changed since `base` and written unless it
/// holds that already, in any format; the list is the next save's base. An unreadable file takes ours.
pub fn save_projects(support: &Path, base: &[Project], ours: &[Project]) -> std::io::Result<Vec<Project>> {
    let theirs = crate::read_json::<ProjectsFile>(&project::file_path(support));
    let merged = match &theirs {
        Some(file) => project::exclusive(&projects(base, ours, &project::exclusive(&file.projects))),
        None => ours.to_vec(),
    };
    if theirs.is_some_and(|file| file.projects == merged) {
        return Ok(merged);
    }
    project::save(support, &ProjectsFile { version: project::FORMAT, projects: merged.clone() })?;
    Ok(merged)
}

#[cfg(test)]
mod tests;
