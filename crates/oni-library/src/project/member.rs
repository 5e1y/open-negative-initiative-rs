//! A project's photo, held by its path and its print: the path tells two copies of one file apart,
//! the print finds the photo again where its path went stale and tells another file standing there.
//! A placeholder 1.5.6 wrote for a print not read yet is a member by that text alone.

use super::Project;
use crate::index::Entry;
use crate::library::WAITING_MARK;
use crate::paths;
use oni_settings::{Date, Id};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Member {
    /// Where the photo was last seen; none for a Swift print no gesture has placed yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The file's fingerprint; none for a photo still being read when it joined, or one a version 1
    /// file named by its path alone; 1.5.6's placeholder text for one it named while its tile loaded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub print: Option<String>,
}

impl Member {
    pub fn new(path: Option<&str>, print: Option<&str>) -> Member {
        Member { path: path.map(str::to_owned), print: print.map(str::to_owned) }
    }

    /// The member a row makes: the row's path and print; a row with no print read yet, a stand-in for
    /// a member no file shows included, is the member it stands for, whole.
    pub fn of(row: &Entry, known: Option<&Member>) -> Member {
        match (read(&row.fingerprint), known) {
            (None, Some(member)) => member.clone(),
            (print, _) => Member { path: Some(row.path.clone()), print: print.map(str::to_owned) },
        }
    }

    /// 1.5.6's placeholder text, which stands at the file at its path whatever print that carries.
    pub fn placeholder(&self) -> Option<&str> {
        self.print.as_deref().filter(|print| print.starts_with(WAITING_MARK))
    }

    /// Whether a row's print could be this photo's: the same, or one side not read yet.
    pub fn agrees(&self, print: &str) -> bool {
        match (self.print.as_deref().and_then(read), read(print)) {
            (Some(mine), Some(theirs)) => mine == theirs,
            _ => true,
        }
    }

    /// Whether two members are one photo: one path, their prints agreeing; one print, neither placed;
    /// two placeholders of one text, as 1.5.6 takes them, a placeholder never one with another member.
    pub fn same(&self, other: &Member) -> bool {
        match (self.placeholder(), other.placeholder()) {
            (None, None) => {}
            (mine, theirs) => return mine == theirs,
        }
        match (&self.path, &other.path) {
            (Some(a), Some(b)) => paths::same(a, b) && other.print.as_deref().is_none_or(|p| self.agrees(p)),
            (None, None) => self.print == other.print,
            _ => false,
        }
    }

    /// The text two members that may be one photo share, which `same` then settles.
    pub(super) fn slot(&self) -> String {
        match (self.placeholder(), &self.path) {
            (Some(text), _) => text.to_owned(),
            (None, Some(path)) => paths::normal(path).to_lowercase(),
            (None, None) => format!("\0{}", self.print.as_deref().unwrap_or_default()),
        }
    }
}

/// A print as read, none for a row still waiting for its own.
pub fn read(print: &str) -> Option<&str> {
    (!print.is_empty() && !print.starts_with(WAITING_MARK)).then_some(print)
}

/// Each photo once: of two members that are one photo the first keeps its place, and its print
/// where it had none, so nothing either recorded is lost.
pub fn distinct(members: Vec<Member>) -> Vec<Member> {
    let mut out: Vec<Member> = Vec::with_capacity(members.len());
    let mut slots: HashMap<String, Vec<usize>> = HashMap::new();
    for member in members {
        let slot = slots.entry(member.slot()).or_default();
        match slot.iter().copied().find(|&i| out[i].same(&member)) {
            Some(i) if out[i].print.is_none() => out[i].print = member.print,
            Some(_) => {}
            None => {
                slot.push(out.len());
                out.push(member);
            }
        }
    }
    out
}

/// `members`, each with a path and no print taking the print its photo carries in `known`: a print
/// read since, so each side of a merge or an undo names the same member alike.
pub fn printed(members: &[Member], known: &[Member]) -> Vec<Member> {
    let holder = holder(known);
    members.iter().map(|m| if m.path.is_some() && m.print.is_none() { Member { print: holder(m).and_then(|k| k.print.clone()), ..m.clone() } } else { m.clone() }).collect()
}

/// The member of `known` holding its print and path that is one photo with the member asked, sought
/// among the few sharing its slot; a placeholder holds no print to lend.
pub fn holder<'a>(known: &'a [Member]) -> impl Fn(&Member) -> Option<&'a Member> + 'a {
    let mut slots: HashMap<String, Vec<&'a Member>> = HashMap::new();
    for k in known.iter().filter(|k| k.path.is_some() && k.print.as_deref().and_then(read).is_some()) {
        slots.entry(k.slot()).or_default().push(k);
    }
    move |m| slots.get(&m.slot()).and_then(|ks| ks.iter().copied().find(|k| m.same(k)))
}

/// One project per photo, the most recently created keeping it: a placeholder by its text alone, as
/// 1.5.6 keeps one, never weighed against a print, which `resolve` lets stand in for it.
pub fn exclusive(projects: &[Project]) -> Vec<Project> {
    let mut slots: HashMap<String, Vec<(Member, Id, Date)>> = HashMap::new();
    for project in projects {
        for member in &project.members {
            let slot = slots.entry(member.slot()).or_default();
            match slot.iter_mut().find(|(m, _, _)| m.same(member)) {
                Some(keeper) if keeper.2 < project.created_at => (keeper.1, keeper.2) = (project.id, project.created_at),
                Some(_) => {}
                None => slot.push((member.clone(), project.id, project.created_at)),
            }
        }
    }
    let keeper = |member: &Member| slots.get(&member.slot()).and_then(|s| s.iter().find(|(m, _, _)| m.same(member))).map(|(_, id, _)| *id);
    projects
        .iter()
        .map(|project| {
            let mut kept = project.clone();
            kept.members.retain(|m| keeper(m) == Some(project.id));
            kept
        })
        .collect()
}
