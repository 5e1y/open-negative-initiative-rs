//! The gestures on projects' members, each on the rows a hand saw: a row is the member it stands
//! for, so a member found by its print moves, joins or leaves as the photo shown does.

use super::member::{Member, distinct};
use super::resolve::{Reach, holders};
use super::{Project, root_order, untitled};
use crate::index::Entry;
use crate::paths::holds;
use oni_settings::{Date, Id};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Each project's members by id, what an Undo step holds at either end of a gesture on them.
pub type Rosters = BTreeMap<Id, Vec<Member>>;

/// The rows picked, in library order, each once.
fn picked<'a>(reach: &Reach<'a>, paths: &HashSet<String>) -> Vec<&'a Entry> {
    let mut seen = HashSet::new();
    reach.rows.iter().filter(|e| paths.contains(&e.path) && seen.insert(e.path.as_str())).collect()
}

/// The members `rows` bring, each leaving the project its row stood in, a photo sitting in one.
fn taken(projects: &mut [Project], rows: &[&Entry], holders: &HashMap<String, (usize, usize)>) -> Vec<Member> {
    let mut leaving: HashSet<(usize, usize)> = HashSet::new();
    let brought = rows
        .iter()
        .map(|row| {
            let held = holders.get(&row.path).copied();
            leaving.extend(held);
            Member::of(row, held.map(|(p, m)| &projects[p].members[m]))
        })
        .collect();
    for (p, project) in projects.iter_mut().enumerate() {
        let mut m = 0;
        project.members.retain(|_| {
            m += 1;
            !leaving.contains(&(p, m - 1))
        });
    }
    brought
}

/// `newProject(from:)`: the photos picked, in library order, under the lowest free name, each
/// leaving its project; none picked makes none.
pub fn new_from(projects: &mut Vec<Project>, reach: &Reach, paths: &HashSet<String>, now: Date) -> Option<Id> {
    let rows = picked(reach, paths);
    if rows.is_empty() {
        return None;
    }
    let holders = holders(projects, reach);
    let members = taken(projects, &rows, &holders);
    let made = Project::new(&untitled(projects), now, members);
    let id = made.id;
    projects.push(made);
    Some(id)
}

/// The empty Projects tab's key: a project holding no photo yet, for Add to Project to fill.
pub fn new_empty(projects: &mut Vec<Project>, now: Date) -> Id {
    let made = Project::new(&untitled(projects), now, Vec::new());
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

/// The projects `add_to` would take photos from, each with how many: what the ask names first.
pub fn leaving<'a>(projects: &'a [Project], reach: &Reach, paths: &HashSet<String>, id: Id) -> Vec<(&'a Project, usize)> {
    let mut counts: HashMap<usize, usize> = HashMap::new();
    for (path, (p, _)) in holders(projects, reach) {
        if paths.contains(&path) && projects[p].id != id {
            *counts.entry(p).or_default() += 1;
        }
    }
    projects.iter().enumerate().filter_map(|(p, project)| counts.get(&p).map(|n| (project, *n))).collect()
}

/// What Add to Project did: the photos that joined, and those already members, left as they were.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Joined {
    pub added: usize,
    pub already: usize,
}

/// `addToProject`: the photos not already members appended in library order, each leaving the
/// project it sat in. A project left empty stays: only Delete Project, which asks, takes one away.
pub fn add_to(projects: &mut [Project], reach: &Reach, paths: &HashSet<String>, id: Id) -> Joined {
    let Some(at) = projects.iter().position(|p| p.id == id) else { return Joined::default() };
    let holders = holders(projects, reach);
    let (already, joining): (Vec<&Entry>, Vec<&Entry>) = picked(reach, paths).into_iter().partition(|row| holders.get(&row.path).is_some_and(|(p, _)| *p == at));
    let done = Joined { added: joining.len(), already: already.len() };
    if !joining.is_empty() {
        let members = taken(projects, &joining, &holders);
        let mut joined = std::mem::take(&mut projects[at].members);
        joined.extend(members);
        projects[at].members = distinct(joined);
    }
    done
}

/// `removeFromProject`: the photos leave it, how many said; a project left empty stays, named, until deleted.
pub fn remove_from(projects: &mut [Project], reach: &Reach, paths: &HashSet<String>, id: Id) -> usize {
    let Some(at) = projects.iter().position(|p| p.id == id) else { return 0 };
    let gone: HashSet<usize> = holders(projects, reach).into_iter().filter(|(path, (p, _))| *p == at && paths.contains(path)).map(|(_, (_, m))| m).collect();
    let mut m = 0;
    projects[at].members.retain(|_| {
        m += 1;
        !gone.contains(&(m - 1))
    });
    gone.len()
}

/// The projects holding any of `paths`, in the file's order.
pub fn member_projects<'a>(projects: &'a [Project], reach: &Reach, paths: &HashSet<String>) -> Vec<&'a Project> {
    let held: HashSet<usize> = holders(projects, reach).into_iter().filter(|(path, _)| paths.contains(path)).map(|(_, (p, _))| p).collect();
    projects.iter().enumerate().filter(|(p, _)| held.contains(p)).map(|(_, project)| project).collect()
}

/// `reorderProjectDrop`: the dragged photos land before `target` in the target's project, those
/// already members first in their own order, the rest joining from other projects.
pub fn reorder_drop(projects: &mut [Project], reach: &Reach, dragged: &HashSet<String>, target: &str) -> bool {
    let holders = holders(projects, reach);
    let Some(&(at, aim)) = holders.get(target) else { return false };
    let target_member = projects[at].members[aim].clone();
    let rows: Vec<&Entry> = picked(reach, dragged).into_iter().filter(|row| row.path != target).collect();
    if rows.is_empty() {
        return false;
    }
    let (inside, outside): (Vec<&Entry>, Vec<&Entry>) = rows.into_iter().partition(|row| holders.get(&row.path).is_some_and(|(p, _)| *p == at));
    let mut own: Vec<usize> = inside.iter().filter_map(|row| holders.get(&row.path).map(|(_, m)| *m)).collect();
    own.sort_unstable();
    let already: Vec<Member> = own.iter().map(|m| projects[at].members[*m].clone()).collect();
    let joining = taken(projects, &outside, &holders);
    let mut members: Vec<Member> = projects[at].members.iter().filter(|m| !already.contains(m)).cloned().collect();
    let Some(insert) = members.iter().position(|m| *m == target_member) else { return false };
    members.splice(insert..insert, already.into_iter().chain(joining));
    projects[at].members = distinct(members);
    true
}

/// `reorderProjectsRoot`: projects sort newest first, so the dragged ones take orders stepping
/// down from the target's towards the sibling above it, or a day above when it heads the list.
pub fn reorder_root(projects: &mut [Project], dragged: &HashSet<Id>, target: Id) -> bool {
    let mut siblings: Vec<(Id, f64)> = projects.iter().map(|p| (p.id, root_order(p))).collect();
    siblings.sort_by(|a, b| b.1.total_cmp(&a.1));
    let Some(target_at) = siblings.iter().position(|(id, _)| *id == target) else { return false };
    let movable: Vec<Id> = siblings.iter().map(|(id, _)| *id).filter(|id| *id != target && dragged.contains(id)).collect();
    if movable.is_empty() {
        return false;
    }
    let ahead = siblings[target_at].1;
    let behind = if target_at > 0 { siblings[target_at - 1].1 } else { ahead + 86_400.0 };
    let step = (ahead - behind) / (movable.len() + 1) as f64;
    for (i, id) in movable.iter().enumerate() {
        if let Some(project) = projects.iter_mut().find(|p| p.id == *id) {
            project.manual_order = Some(ahead - step * (i + 1) as f64);
        }
    }
    true
}

/// `path` carried by the first of `moves` holding it, else as it is.
pub fn carried(path: &str, moves: &[(String, String)]) -> String {
    moves.iter().find(|(from, _)| holds(from, path)).map_or_else(|| path.to_owned(), |(from, to)| crate::paths::moved(path, from, to))
}

/// A member carried by `moves`, its print kept.
pub fn moved_member(member: &Member, moves: &[(String, String)]) -> Member {
    Member { path: member.path.as_deref().map(|p| carried(p, moves)), print: member.print.clone() }
}

/// A folder or a file moved in ONI carries the members under it, each keeping its print; two that
/// land on one photo become one.
pub fn moved(projects: &[Project], moves: &[(String, String)]) -> Vec<Project> {
    let carry = |project: &Project| distinct(project.members.iter().map(|m| moved_member(m, moves)).collect());
    projects.iter().map(|project| Project { members: carry(project), ..project.clone() }).collect()
}

pub fn rosters(projects: &[Project]) -> Rosters {
    projects.iter().map(|p| (p.id, p.members.clone())).collect()
}

/// An undo or a redo of a step that took projects `from` to `to`: what it changed, laid on each as it
/// stands, a member it never touched kept; a project absent at one end held none there.
pub fn regroup(projects: &mut [Project], from: &Rosters, to: &Rosters) {
    let none = Vec::new();
    for project in projects.iter_mut() {
        let (base, ours) = (from.get(&project.id), to.get(&project.id));
        if base.is_none() && ours.is_none() {
            continue;
        }
        let laid = crate::merge::list(base.unwrap_or(&none), ours.unwrap_or(&none), &project.members);
        project.members = distinct(laid);
    }
}
