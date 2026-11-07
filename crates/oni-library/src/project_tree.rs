//! The projects' sidebar tree: a project opens into its rolls, each heading its own photos.

use crate::index::Entry;
use crate::paths;
use crate::project::{self, Project, ProjectGroup, Reach};
use oni_settings::Id;
use std::collections::HashSet;

/// A photo's identity carries its project: the same photo can sit under two open projects.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NodeId {
    Project(Id),
    /// A roll's heading inside a project, by its folder.
    Roll { folder: String, project: Id },
    Photo { path: String, project: Id },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Project(ProjectGroup),
    Roll { folder: String, project: Id, entries: Vec<Entry> },
    Photo { entry: Entry, project: Id },
}

impl Node {
    pub fn id(&self) -> NodeId {
        match self {
            Node::Project(group) => NodeId::Project(group.id),
            Node::Roll { folder, project, .. } => NodeId::Roll { folder: folder.clone(), project: *project },
            Node::Photo { entry, project } => NodeId::Photo { path: entry.path.clone(), project: *project },
        }
    }

    pub fn entries(&self) -> Vec<Entry> {
        match self {
            Node::Project(group) => group.entries.clone(),
            Node::Roll { entries, .. } => entries.clone(),
            Node::Photo { entry, .. } => vec![entry.clone()],
        }
    }
}

/// A project's photos by roll, the folder each lies in: the rolls in the order their first photo
/// takes in the project, each keeping the project's order; two copies of one file stay apart.
pub fn by_roll(entries: &[Entry]) -> Vec<(String, Vec<Entry>)> {
    let mut rolls: Vec<(String, Vec<Entry>)> = Vec::new();
    for entry in entries {
        let folder = paths::parent(&entry.path);
        match rolls.iter_mut().find(|(f, _)| *f == folder) {
            Some((_, photos)) => photos.push(entry.clone()),
            None => rolls.push((folder, vec![entry.clone()])),
        }
    }
    rolls
}

/// Every project, newest first; a photo in no project is absent.
pub fn roots(projects: &[Project], reach: &Reach) -> Vec<Node> {
    project::grouped(projects, reach).into_iter().map(Node::Project).collect()
}

/// An open project lays each roll's heading a level down, its photos a level further.
pub fn flatten(nodes: &[Node], expanded: &HashSet<NodeId>, depth: usize) -> Vec<(Node, usize)> {
    let mut rows = Vec::new();
    for node in nodes {
        rows.push((node.clone(), depth));
        if let Node::Project(group) = node
            && expanded.contains(&node.id())
        {
            for (folder, entries) in by_roll(&group.entries) {
                let photos: Vec<(Node, usize)> = entries.iter().map(|e| (Node::Photo { entry: e.clone(), project: group.id }, depth + 2)).collect();
                rows.push((Node::Roll { folder, project: group.id, entries }, depth + 1));
                rows.extend(photos);
            }
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::Member;
    use oni_settings::Date;

    fn at(paths: &[&str]) -> Vec<Member> {
        paths.iter().map(|p| Member::new(Some(p), None)).collect()
    }

    #[test]
    fn a_project_opens_into_its_rolls_each_heading_its_photos_tagged_with_its_project() {
        let base = Date::since_reference_date(8e8);
        let library = [Entry::new("/negs/a.RW2", base, "fp-a"), Entry::new("/negs/b.RW2", base, "fp-b")];
        let older = Project::new("Older", base, at(&["/negs/a.RW2"]));
        let newer = Project::new("Newer", crate::later(base, 3600.0), at(&["/negs/b.RW2"]));
        let tops = roots(&[older, newer.clone()], &Reach::rows(&library));
        assert_eq!(tops.len(), 2);
        assert_eq!(tops[0].id(), NodeId::Project(newer.id));
        assert_eq!(flatten(&tops, &HashSet::new(), 0).len(), 2);
        let open = flatten(&tops, &HashSet::from([NodeId::Project(newer.id)]), 0);
        assert_eq!(open.len(), 4);
        assert_eq!((open[1].0.id(), open[1].1), (NodeId::Roll { folder: "/negs".into(), project: newer.id }, 1));
        assert_eq!((open[2].0.id(), open[2].1), (NodeId::Photo { path: "/negs/b.RW2".into(), project: newer.id }, 2));

        let a = Project::new("Shared A", base, at(&["/negs/a.RW2"]));
        let b = Project::new("Shared B", crate::later(base, 60.0), at(&["/negs/a.RW2"]));
        let both = flatten(
            &roots(&[a.clone(), b.clone()], &Reach::rows(&library)),
            &HashSet::from([NodeId::Project(a.id), NodeId::Project(b.id)]),
            0,
        );
        let rows: Vec<NodeId> =
            both.iter().filter(|(n, _)| matches!(n, Node::Photo { .. })).map(|(n, _)| n.id()).collect();
        assert_eq!(rows.len(), 2);
        // Twin: the two rows of one photo carry distinct ids, one per project.
        assert_ne!(rows[0], rows[1]);
        let gap = Project::new("Gap", base, at(&["/negs/a.RW2", "/negs/missing.RW2"]));
        assert_eq!(
            flatten(&roots(std::slice::from_ref(&gap), &Reach::rows(&library)), &HashSet::from([NodeId::Project(gap.id)]), 0).len(),
            3
        );
    }

    #[test]
    fn two_rolls_copies_of_one_file_stand_under_their_own_roll_in_the_project_s_order() {
        let base = Date::since_reference_date(8e8);
        let entry = |p: &str| Entry::new(p, base, &format!("fp-{p}"));
        let order = [entry("/s/Roll 10/A.NEF"), entry("/s/Roll 8/A.NEF"), entry("/s/Roll 10/C.NEF"), entry("/s/Roll 8/B.NEF")];
        let rolls = by_roll(&order);
        let named: Vec<(&str, Vec<&str>)> =
            rolls.iter().map(|(f, e)| (f.as_str(), e.iter().map(|e| e.path.as_str()).collect())).collect();
        assert_eq!(
            named,
            [("/s/Roll 10", vec!["/s/Roll 10/A.NEF", "/s/Roll 10/C.NEF"]), ("/s/Roll 8", vec!["/s/Roll 8/A.NEF", "/s/Roll 8/B.NEF"])],
            "Roll 10 first, its first photo leading the project"
        );
        // Twin: grouped by file name, the two A.NEF would share one roll.
        assert_ne!(rolls[0].1.iter().filter(|e| e.path.ends_with("/A.NEF")).count(), 2);
    }
}
