//! The gestures on projects' members, each on the rows a hand saw: a row is the member it stands
//! for, so a member found by its print moves, joins or leaves as the photo shown does.

use super::member::{Member, distinct, printed};
use super::resolve::{Reach, holders, place, resolve, showing};
use super::{Project, root_order, untitled};
use crate::index::Entry;
use crate::paths::{self, holds};
use crate::project_tree::rolls;
use oni_settings::{Date, Id};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Each project a gesture changed, by id, as it stood at one end: what an Undo step holds. One
/// absent at an end did not stand there, as a project New Project makes.
pub type Rosters = BTreeMap<Id, Project>;

/// The rows picked, in library order, each once.
fn picked<'a>(reach: &Reach<'a>, paths: &HashSet<String>) -> Vec<&'a Entry> {
    let mut seen = HashSet::new();
    reach.rows.iter().filter(|e| paths.contains(&e.path) && seen.insert(e.path.as_str())).collect()
}

/// Each placeholder by the path it stands at: the photo's own there, unless another member stands in for it.
fn waiting<'a>(projects: &'a [Project], reach: &'a Reach) -> HashMap<&'a str, Vec<(usize, usize)>> {
    let mut out: HashMap<&str, Vec<(usize, usize)>> = HashMap::new();
    for (p, project) in projects.iter().enumerate() {
        for (m, member) in project.members.iter().enumerate().filter(|(_, member)| member.placeholder().is_some()) {
            out.entry(place(member, reach).unwrap_or_default()).or_default().push((p, m));
        }
    }
    out
}

/// The members `rows` bring, and the place of every member standing for their photos: the one each
/// row shows, and each placeholder at its path, so none brings the photo back where it left.
fn taking(projects: &[Project], reach: &Reach, rows: &[&Entry], holders: &HashMap<String, (usize, usize)>) -> (Vec<Member>, HashSet<(usize, usize)>) {
    let waiting = waiting(projects, reach);
    let mut leaving: HashSet<(usize, usize)> = HashSet::new();
    let brought = rows
        .iter()
        .map(|row| {
            let held = holders.get(&row.path).copied();
            leaving.extend(held);
            leaving.extend(waiting.get(row.path.as_str()).into_iter().flatten());
            Member::of(row, held.map(|(p, m)| &projects[p].members[m]))
        })
        .collect();
    (brought, leaving)
}

fn leave(projects: &mut [Project], leaving: &HashSet<(usize, usize)>) {
    for (p, project) in projects.iter_mut().enumerate() {
        let mut m = 0;
        project.members.retain(|_| {
            m += 1;
            !leaving.contains(&(p, m - 1))
        });
    }
}

/// The members `rows` bring, each photo leaving every project it stood in, as a photo sits in one.
fn taken(projects: &mut [Project], reach: &Reach, rows: &[&Entry], holders: &HashMap<String, (usize, usize)>) -> Vec<Member> {
    let (brought, leaving) = taking(projects, reach, rows, holders);
    leave(projects, &leaving);
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
    let members = taken(projects, reach, &rows, &holders);
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

/// The projects `add_to` would take photos from, each with how many: what the ask names first. A photo
/// the project already holds takes nothing; another leaves every project showing it.
pub fn leaving<'a>(projects: &'a [Project], reach: &Reach, paths: &HashSet<String>, id: Id) -> Vec<(&'a Project, usize)> {
    let joining = |path: &String, members: &[(usize, usize)]| paths.contains(path) && members.first().is_some_and(|(p, _)| projects[*p].id != id);
    let shown = showing(projects, reach).into_iter().filter(|(path, members)| joining(path, members));
    counted(projects, shown.map(|(_, members)| members.into_iter().filter(|(p, _)| projects[*p].id != id).collect()))
}

/// Each project with how many of the photos show in it, `members` naming each photo's, in the file's order.
fn counted(projects: &[Project], members: impl Iterator<Item = Vec<(usize, usize)>>) -> Vec<(&Project, usize)> {
    let mut counts: HashMap<usize, usize> = HashMap::new();
    for shown in members {
        for p in shown.into_iter().map(|(p, _)| p).collect::<HashSet<usize>>() {
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
        let members = taken(projects, reach, &joining, &holders);
        let mut joined = std::mem::take(&mut projects[at].members);
        joined.extend(members);
        projects[at].members = distinct(joined);
    }
    done
}

/// `removeFromProject`: the photos it shows leave it, with its placeholders at their files; the paths
/// gone, sorted. One another project shows stays there. A project left empty stays, named, until deleted.
pub fn remove_from(projects: &mut [Project], reach: &Reach, paths: &HashSet<String>, id: Id) -> Vec<String> {
    let Some(at) = projects.iter().position(|p| p.id == id) else { return Vec::new() };
    let waiting = waiting(projects, reach);
    let (mut gone, mut leaving) = (Vec::new(), HashSet::new());
    for (path, members) in showing(projects, reach).into_iter().filter(|(path, _)| paths.contains(path)) {
        let mine: Vec<(usize, usize)> = members.into_iter().filter(|(p, _)| *p == at).collect();
        if !mine.is_empty() {
            leaving.extend(mine.into_iter().chain(waiting.get(path.as_str()).into_iter().flatten().copied().filter(|(p, _)| *p == at)));
            gone.push(path);
        }
    }
    leave(projects, &leaving);
    gone.sort();
    gone
}

/// The projects showing any of `paths`, in the file's order.
pub fn member_projects<'a>(projects: &'a [Project], reach: &Reach, paths: &HashSet<String>) -> Vec<&'a Project> {
    shown_in(projects, reach, paths).into_iter().map(|(project, _)| project).collect()
}

/// The projects showing any of `paths`, each with how many, in the file's order.
pub fn shown_in<'a>(projects: &'a [Project], reach: &Reach, paths: &HashSet<String>) -> Vec<(&'a Project, usize)> {
    counted(projects, showing(projects, reach).into_iter().filter(|(path, _)| paths.contains(path)).map(|(_, members)| members))
}

/// Whether photos dropped beside `target` stand there: a project shows each roll apart, so only
/// photos of the target's roll land beside it.
pub fn lands_beside<'a>(dragged: impl IntoIterator<Item = &'a String>, target: &str) -> bool {
    let roll = paths::parent(target);
    dragged.into_iter().all(|path| paths::parent(path) == roll)
}

/// `reorderProjectDrop`: the dragged photos land before `target`, or past it, in its project's
/// order as shown, roll by roll, which is written back; those already members first, in that order.
pub fn reorder_drop(projects: &mut [Project], reach: &Reach, dragged: &HashSet<String>, target: &str, past: bool) -> bool {
    let (holders, shown) = (holders(projects, reach), showing(projects, reach));
    // A tile two projects show, as one whose file is not read yet, names no one project to land in.
    let Some(&[(at, aim)]) = shown.get(target).map(Vec::as_slice) else { return false };
    let rows = picked(reach, dragged);
    if rows.is_empty() || !lands_beside(rows.iter().map(|row| &row.path), target) {
        return false;
    }
    let mine: HashMap<&str, usize> = shown.iter().filter_map(|(path, members)| Some((path.as_str(), members.iter().find(|(p, _)| *p == at)?.1))).collect();
    let (inside, outside): (Vec<&Entry>, Vec<&Entry>) = rows.into_iter().partition(|row| mine.contains_key(row.path.as_str()));
    let moving: HashSet<usize> = inside.iter().filter_map(|row| mine.get(row.path.as_str()).copied()).collect();
    let shown: Vec<(usize, &Entry)> = resolve(projects, reach).swap_remove(at).into_iter().enumerate().filter_map(|(m, row)| Some((m, row?))).collect();
    let order: Vec<usize> = rolls(shown, |(_, row)| paths::parent(&row.path)).into_iter().flat_map(|(_, photos)| photos).map(|(m, _)| m).collect();
    let Some(aimed) = order.iter().position(|m| *m == aim) else { return false };
    let (joining, leaving) = taking(projects, reach, &outside, &holders);
    let members = &projects[at].members;
    let end = aimed + usize::from(past);
    let place = end - order[..end].iter().filter(|m| moving.contains(m)).count();
    let mut laid: Vec<Member> = order.iter().filter(|m| !moving.contains(m)).map(|m| members[*m].clone()).collect();
    let brought: Vec<Member> = order.iter().filter(|m| moving.contains(m)).map(|m| members[*m].clone()).chain(joining).collect();
    laid.splice(place..place, brought);
    // Each place a photo shows takes the next of the order laid; a member none shows keeps its own,
    // but a placeholder at a joining photo's file.
    let placed: HashSet<usize> = order.iter().copied().collect();
    let mut next = laid.into_iter();
    let kept = members.iter().enumerate().filter(|(m, _)| !leaving.contains(&(at, *m)));
    let mut written: Vec<Member> = kept.filter_map(|(m, member)| if placed.contains(&m) { next.next() } else { Some(member.clone()) }).collect();
    written.extend(next);
    let written = distinct(written);
    let moved = !outside.is_empty() || written != projects[at].members;
    leave(projects, &leaving);
    projects[at].members = written;
    moved
}

/// `reorderProjectsRoot`: projects sort newest first, so the dragged ones take orders stepping
/// down from the target's towards the sibling above it, or a day above when it heads the list;
/// a drop that leaves the order as it stands writes nothing.
pub fn reorder_root(projects: &mut [Project], dragged: &HashSet<Id>, target: Id) -> bool {
    let descending = |list: &mut Vec<(Id, f64)>| list.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut siblings: Vec<(Id, f64)> = projects.iter().map(|p| (p.id, root_order(p))).collect();
    descending(&mut siblings);
    let Some(target_at) = siblings.iter().position(|(id, _)| *id == target) else { return false };
    let movable: Vec<Id> = siblings.iter().map(|(id, _)| *id).filter(|id| *id != target && dragged.contains(id)).collect();
    let ahead = siblings[target_at].1;
    let behind = if target_at > 0 { siblings[target_at - 1].1 } else { ahead + 86_400.0 };
    let step = (ahead - behind) / (movable.len() + 1) as f64;
    let placed: HashMap<Id, f64> = movable.iter().enumerate().map(|(i, id)| (*id, ahead - step * (i + 1) as f64)).collect();
    let mut after: Vec<(Id, f64)> = siblings.iter().map(|(id, order)| (*id, placed.get(id).copied().unwrap_or(*order))).collect();
    descending(&mut after);
    if after.iter().map(|(id, _)| id).eq(siblings.iter().map(|(id, _)| id)) {
        return false;
    }
    for project in projects.iter_mut() {
        if let Some(order) = placed.get(&project.id) {
            project.manual_order = Some(*order);
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

/// A folder or a file moved in ONI carries the members under it, each keeping its print, a placeholder
/// its text as 1.5.6 keeps it; two that land on one photo become one.
pub fn moved(projects: &[Project], moves: &[(String, String)]) -> Vec<Project> {
    let carry = |project: &Project| distinct(project.members.iter().map(|m| moved_member(m, moves)).collect());
    projects.iter().map(|project| Project { members: carry(project), ..project.clone() }).collect()
}

pub fn rosters(projects: &[Project]) -> Rosters {
    projects.iter().map(|p| (p.id, p.clone())).collect()
}

/// An undo or a redo of a step from `from` to `to`, laid on the projects as they stand: a project it
/// made goes while empty and comes back, a photo bound for a project deleted since stays where it is.
pub fn regroup(projects: &mut Vec<Project>, from: &Rosters, to: &Rosters) {
    for (id, made) in to.iter().filter(|(id, _)| !from.contains_key(*id)) {
        if !projects.iter().any(|p| p.id == *id) {
            projects.push(Project { members: Vec::new(), ..made.clone() });
        }
    }
    let ends = from.values().chain(to.values()).flat_map(|p| &p.members);
    let known: Vec<Member> = projects.iter().flat_map(|p| &p.members).chain(ends).cloned().collect();
    let side = |end: &Rosters, id: &Id| end.get(id).map(|p| printed(&p.members, &known));
    let homeless: Vec<Member> = to.keys().filter(|id| !projects.iter().any(|p| p.id == **id)).flat_map(|id| side(to, id).unwrap_or_default()).collect();
    for project in projects.iter_mut() {
        let (base, ours) = (side(from, &project.id), side(to, &project.id));
        if base.is_none() && ours.is_none() {
            continue;
        }
        let base: Vec<Member> = base.unwrap_or_default().into_iter().filter(|m| !homeless.contains(m)).collect();
        let laid = crate::merge::list(&base, &ours.unwrap_or_default(), &printed(&project.members, &known));
        project.members = distinct(laid);
    }
    projects.retain(|p| !(from.contains_key(&p.id) && !to.contains_key(&p.id) && p.members.is_empty()));
}
