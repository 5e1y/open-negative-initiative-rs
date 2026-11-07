//! A member only the file at its path tells from another: 1.5.6's placeholder, or any member with no
//! print, at a path a print also holds. It is no member until that file is read, and is written back as given.

use super::Project;
use super::member::{Member, distinct};
use crate::library::WAITING_MARK;
use crate::paths;
use std::collections::{BTreeMap, HashMap};

/// Held apart from its project's members, so no gesture, undo, prompt or menu reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct Unsettled {
    /// The text 1.5.6 gave it: its placeholder's mark and the path it named then.
    pub text: String,
    /// The path recorded beside it, which a folder moved in ONI carries.
    pub path: String,
    /// The prints held at that path when it was read: a file carrying one of them is their photo.
    pub against: Vec<String>,
    /// How many members stand before it.
    pub at: usize,
}

/// A project as read: its members in order, each with the placeholder text it was read from, and the
/// prints this format recorded against each text it held apart.
pub(super) struct Read {
    pub project: Project,
    pub members: Vec<(Member, Option<String>)>,
    pub listed: BTreeMap<String, Vec<String>>,
}

/// The projects read, each member with a path and no print, where a print stands at that path in any
/// project, held apart against the prints this format recorded for it, else against those standing there.
pub(super) fn hold(read: Vec<Read>) -> Vec<Project> {
    let mut printed: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for (m, _) in read.iter().flat_map(|r| &r.members) {
        if let (Some(path), Some(print)) = (&m.path, &m.print) {
            printed.entry(m.slot()).or_default().push((path.clone(), print.clone()));
        }
    }
    let standing = |m: &Member, path: &str| {
        let mut prints: Vec<String> = printed.get(&m.slot()).into_iter().flatten().filter(|(p, _)| paths::same(p, path)).map(|(_, print)| print.clone()).collect();
        prints.sort();
        prints.dedup();
        prints
    };
    let mut out = Vec::with_capacity(read.len());
    for Read { mut project, members, listed } in read {
        let mut kept = Vec::with_capacity(members.len());
        for (member, text) in members {
            let (Some(path), None) = (member.path.as_deref(), member.print.as_deref()) else {
                kept.push(member);
                continue;
            };
            let against = text.as_ref().and_then(|t| listed.get(t)).cloned().unwrap_or_else(|| standing(&member, path));
            if against.is_empty() {
                kept.push(member);
                continue;
            }
            let text = text.unwrap_or_else(|| format!("{WAITING_MARK}{path}"));
            project.unsettled.push(Unsettled { text, path: path.to_owned(), against, at: kept.len() });
        }
        for u in &mut project.unsettled {
            u.at = distinct(kept[..u.at].to_vec()).len();
        }
        project.members = distinct(kept);
        out.push(project);
    }
    out
}

/// Each unsettled member whose file `carried` reads: dropped where that file carries a print held against
/// it or by a member there, the photo staying where it stands, else a member at its place with that print.
pub fn settle(projects: &mut [Project], carried: impl Fn(&str) -> Option<String>) -> bool {
    let mut known: Vec<Member> = projects.iter().flat_map(|p| &p.members).filter(|m| m.path.is_some() && m.print.is_some()).cloned().collect();
    let mut changed = false;
    for project in projects.iter_mut() {
        for u in std::mem::take(&mut project.unsettled) {
            let Some(print) = carried(&u.path) else {
                project.unsettled.push(u);
                continue;
            };
            changed = true;
            let learned = Member::new(Some(&u.path), Some(&print));
            if u.against.contains(&print) || known.iter().any(|k| k.same(&learned)) {
                continue;
            }
            project.members.insert(u.at.min(project.members.len()), learned.clone());
            known.push(learned);
        }
    }
    changed
}
