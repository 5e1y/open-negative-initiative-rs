//! A project's members wherever they lie, as Swift's `standing`, found again by their prints where
//! their paths went stale, so a project never shrinks unseen; each read looks again, writing nothing.

use crate::index::{Entry, LibraryIndex};
use crate::library::WAITING_MARK;
use crate::paths;
use crate::project::{self, Found, Member, Project};
use std::collections::{HashMap, HashSet};

/// The rows `project::grouped` needs beyond the opened folder's, and where each member its path no
/// longer shows stands this read.
#[derive(Debug, Default, PartialEq)]
pub struct Standing {
    pub rows: Vec<Entry>,
    pub found: Found,
}

/// What a member looked for by its print left at its own place.
enum Left {
    /// No path: a Swift print no gesture has placed.
    Unplaced,
    /// No file at its path.
    Gone,
    /// Another file at its path.
    Replaced,
}

/// A member stands at its path while the row there carries its print, a trashed one hidden; else on a
/// free copy, the one `last` chose while free, then the nearest by name and folder; else missing.
pub fn standing(projects: &[Project], held: &[Entry], index: &LibraryIndex, last: &Found) -> Standing {
    let held_at: HashMap<&str, &Entry> = held.iter().map(|e| (e.path.as_str(), e)).collect();
    let mut out = Standing::default();
    let mut added: HashSet<String> = HashSet::new();
    let add = |e: Entry, out: &mut Standing, added: &mut HashSet<String>| {
        if !held_at.contains_key(e.path.as_str()) && added.insert(e.path.clone()) {
            out.rows.push(e);
        }
    };
    let mut claimed: HashSet<String> = HashSet::new();
    let mut looked: Vec<(&Member, Left)> = Vec::new();
    for member in projects.iter().flat_map(|p| &p.members) {
        let Some(path) = member.path.as_deref() else {
            looked.push((member, Left::Unplaced));
            continue;
        };
        match held_at.get(path).map(|e| (*e).clone()).or_else(|| index.row(path)) {
            Some(row) if member.agrees(&row.fingerprint) => {
                claimed.insert(row.path.clone());
                if row.path != path {
                    out.found.insert(member.clone(), row.path.clone());
                }
                if shown(&row, index) {
                    add(row, &mut out, &mut added);
                }
            }
            Some(_) => looked.push((member, Left::Replaced)),
            None => looked.push((member, Left::Gone)),
        }
    }
    let copies: Vec<Vec<Entry>> = looked
        .iter()
        .map(|(member, _)| member.print.as_deref().map(|print| index.with_fingerprint(print)).unwrap_or_default().into_iter().filter(|r| shown(r, index)).collect())
        .collect();
    // The copy each stood on last read stays its own while free, before any other member chooses.
    let mut chosen: Vec<Option<&Entry>> = vec![None; looked.len()];
    for (i, (member, _)) in looked.iter().enumerate() {
        let again = last.get(*member).and_then(|path| copies[i].iter().find(|r| &r.path == path));
        if let Some(row) = again.filter(|r| claimed.insert(r.path.clone())) {
            chosen[i] = Some(row);
        }
    }
    for (i, (member, left)) in looked.iter().enumerate() {
        let row = chosen[i].or_else(|| {
            let free = copies[i].iter().filter(|r| !claimed.contains(&r.path));
            let best = free.enumerate().min_by_key(|(at, r)| (kinship(member.path.as_deref(), &r.path), *at)).map(|(_, r)| r)?;
            claimed.insert(best.path.clone());
            Some(best)
        });
        let at = match (row, left, member.path.as_deref()) {
            (Some(row), _, _) => {
                add(row.clone(), &mut out, &mut added);
                row.path.clone()
            }
            (None, Left::Gone, Some(path)) => {
                add(Entry::new(path, crate::distant_past(), &format!("{WAITING_MARK}{path}")), &mut out, &mut added);
                continue;
            }
            (None, Left::Replaced, Some(path)) => lost(path, &paths::name(path), &added),
            (None, _, _) => match copies[i].first() {
                Some(copy) => lost(&copy.path, &paths::name(&copy.path), &added),
                None => continue,
            },
        };
        if row.is_none() {
            add(Entry::new(&at, crate::distant_past(), &format!("{WAITING_MARK}{at}")), &mut out, &mut added);
        }
        out.found.insert((*member).clone(), at);
    }
    out
}

/// How near a copy lies to a member's own place: its path, then its file name, then its folder.
fn kinship(own: Option<&str>, copy: &str) -> u8 {
    let Some(own) = own else { return 3 };
    if paths::same(own, copy) {
        0
    } else if paths::name(own).to_lowercase() == paths::name(copy).to_lowercase() {
        1
    } else if paths::same(&paths::parent(own), &paths::parent(copy)) {
        2
    } else {
        3
    }
}

/// Where a member no file shows stands when its place holds another file, or its print only rows
/// other members hold: under such a file, a path no file can have, ending in that file's name.
fn lost(file: &str, name: &str, taken: &HashSet<String>) -> String {
    let mut at = paths::join(file, name);
    let mut n = 2;
    while taken.contains(&at) {
        at = paths::join(&paths::join(file, &n.to_string()), name);
        n += 1;
    }
    at
}

/// Each member with a path and no print takes the print its file there carries, or gives the photo up
/// to a member holding that print there, as that print was read after it was named. Whether any changed.
pub fn learn(projects: &mut [Project], held: &[Entry], index: &LibraryIndex) -> bool {
    let held_at: HashMap<&str, &Entry> = held.iter().map(|e| (e.path.as_str(), e)).collect();
    let carried = |path: &str| held_at.get(path).map(|e| (*e).clone()).or_else(|| index.row(path)).and_then(|row| Member::of(&row, None).print);
    let printed: Vec<Member> = projects.iter().flat_map(|p| &p.members).filter(|m| m.print.is_some()).cloned().collect();
    let holder = project::holder(&printed);
    let mut changed = false;
    for project in projects.iter_mut() {
        project.members.retain_mut(|member| {
            let Some(path) = member.path.as_deref().filter(|_| member.print.is_none()) else { return true };
            let Some(print) = carried(path) else { return true };
            let learned = Member::new(Some(path), Some(&print));
            changed = true;
            let kept = holder(&learned).is_none();
            if kept {
                *member = learned;
            }
            kept
        });
    }
    changed
}

/// `LibraryIndex.shown`'s cut on one row: out of the ONI Trash, and no pass whose lead is held.
fn shown(row: &Entry, index: &LibraryIndex) -> bool {
    row.trashed_at.is_none()
        && row.pass_of.as_ref().is_none_or(|lead| index.row(&paths::join(&paths::parent(&row.path), lead)).is_none())
}

#[cfg(test)]
mod tests;
