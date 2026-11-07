//! A curated cross-roll selection laid over the library's order. Membership lives in
//! `projects.json`, not in a sidecar: an ordered member list cannot fit one sidecar field.

use oni_settings::{Date, Id};
use std::collections::HashSet;

mod file;
mod gestures;
mod member;
mod resolve;

pub(crate) use file::read;
pub use file::{FORMAT, ProjectsFile, encode, file_path, former_path, is_path, load, save};
pub use gestures::{
    Joined, Rosters, add_to, carried, delete, lands_beside, leaving, member_projects, moved, moved_member, new_empty, new_from, regroup, remove_from,
    rename, reorder_drop, reorder_root, rosters, set_description, shown_in,
};
pub use member::{Member, distinct, exclusive, holder, learnt, printed};
pub use resolve::{Found, ProjectGroup, Reach, grouped, holders, resolve, shadowed};

#[derive(Debug, Clone, PartialEq)]
pub struct Project {
    pub id: Id,
    pub name: String,
    pub description: String,
    pub created_at: Date,
    pub manual_order: Option<f64>,
    /// Ordered photos, each by its path and its print: two copies of one file in two rolls are two.
    pub members: Vec<Member>,
}

impl Project {
    pub fn new(name: &str, created_at: Date, members: Vec<Member>) -> Project {
        Project { id: Id::new(), name: name.to_owned(), description: String::new(), created_at, manual_order: None, members }
    }

    /// The paths its members were last seen at, in order; a member never placed has none.
    pub fn paths(&self) -> Vec<&str> {
        self.members.iter().filter_map(|m| m.path.as_deref()).collect()
    }
}

pub const UNTITLED_STEM: &str = "Project ";

pub fn untitled(taken: &[Project]) -> String {
    let numbers: HashSet<i64> = taken.iter().filter_map(|p| p.name.strip_prefix(UNTITLED_STEM).and_then(|n| n.parse().ok())).collect();
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

/// `deleteProjectRequested`: the confirmation's title, message and key; its `shadowed` placeholders,
/// whose photos another project shows, are told apart, as no photo of its own.
pub fn delete_prompt(project: &Project, shadowed: usize) -> (String, String, &'static str) {
    let count = project.members.len().saturating_sub(shadowed);
    let message = if count > 0 {
        oni_i18n::trn(
            "It stops grouping its {n} photo. The photos, their settings and every other project or roll they belong to are untouched.",
            "It stops grouping its {n} photos. The photos, their settings and every other project or roll they belong to are untouched.",
            count as u64,
        )
    } else if shadowed > 0 {
        oni_i18n::trn(
            "It shows no photo: the one it names stands in another project, which keeps it. Nothing else is affected.",
            "It shows no photo: the {n} it names stand in other projects, which keep them. Nothing else is affected.",
            shadowed as u64,
        )
    } else {
        oni_i18n::tr("It holds no photos. Nothing else is affected.").to_owned()
    };
    (oni_i18n::tr!("Delete \u{201c}{name}\u{201d}?", name = project.name), message, oni_i18n::tr("Delete Project"))
}

/// What Remove from Project says: how many photos left `project`, then each project any still shows in,
/// so none seems gone that another project shows.
pub fn removed_said(project: &str, removed: usize, showing: &[(&Project, usize)]) -> String {
    let said = oni_i18n::trn("One photo removed from \u{201c}{project}\u{201d}.", "{n} photos removed from \u{201c}{project}\u{201d}.", removed as u64);
    let shows = showing.iter().map(|(other, n)| {
        let line = match removed {
            1 => oni_i18n::tr("It now shows in \u{201c}{project}\u{201d}.").to_owned(),
            _ => oni_i18n::trn("One of them now shows in \u{201c}{project}\u{201d}.", "{n} of them now show in \u{201c}{project}\u{201d}.", *n as u64),
        };
        line.replace("{project}", &other.name)
    });
    std::iter::once(said.replace("{project}", project)).chain(shows).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod file_tests;
#[cfg(test)]
mod shipped_tests;
#[cfg(test)]
mod drop_tests;
#[cfg(test)]
mod placeholder_tests;
