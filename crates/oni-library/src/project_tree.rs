//! The projects' sidebar tree: a project opens onto its photos, one run in its own order, whatever
//! roll each lies in.

use crate::index::Entry;
use crate::project::{self, Project, ProjectGroup, Reach};
use oni_settings::Id;
use std::collections::HashSet;

/// A photo's identity carries its project: the same photo can sit under two open projects.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NodeId {
    Project(Id),
    Photo { path: String, project: Id },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Project(ProjectGroup),
    Photo { entry: Entry, project: Id },
}

impl Node {
    pub fn id(&self) -> NodeId {
        match self {
            Node::Project(group) => NodeId::Project(group.id),
            Node::Photo { entry, project } => NodeId::Photo { path: entry.path.clone(), project: *project },
        }
    }
}

/// Every project, newest first; a photo in no project is absent.
pub fn roots(projects: &[Project], reach: &Reach) -> Vec<Node> {
    project::grouped(projects, reach).into_iter().map(Node::Project).collect()
}

/// An open project lays its photos a level down, in its order.
pub fn flatten(nodes: &[Node], expanded: &HashSet<NodeId>, depth: usize) -> Vec<(Node, usize)> {
    let mut rows = Vec::new();
    for node in nodes {
        rows.push((node.clone(), depth));
        if let Node::Project(group) = node
            && expanded.contains(&node.id())
        {
            rows.extend(group.entries.iter().map(|e| (Node::Photo { entry: e.clone(), project: group.id }, depth + 1)));
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
    fn a_project_opens_onto_its_photos_tagged_with_its_project() {
        let base = Date::since_reference_date(8e8);
        let library = [Entry::new("/negs/a.RW2", base, "fp-a"), Entry::new("/negs/b.RW2", base, "fp-b")];
        let older = Project::new("Older", base, at(&["/negs/a.RW2"]));
        let newer = Project::new("Newer", crate::later(base, 3600.0), at(&["/negs/b.RW2"]));
        let tops = roots(&[older, newer.clone()], &Reach::rows(&library));
        assert_eq!(tops.len(), 2);
        assert_eq!(tops[0].id(), NodeId::Project(newer.id));
        assert_eq!(flatten(&tops, &HashSet::new(), 0).len(), 2);
        let open = flatten(&tops, &HashSet::from([NodeId::Project(newer.id)]), 0);
        assert_eq!(open.len(), 3);
        assert_eq!((open[1].0.id(), open[1].1), (NodeId::Photo { path: "/negs/b.RW2".into(), project: newer.id }, 1));

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
            2
        );
    }

    #[test]
    fn a_project_s_photos_read_in_its_own_order_across_rolls() {
        let base = Date::since_reference_date(8e8);
        let paths = ["/s/Roll 10/A.NEF", "/s/Roll 8/A.NEF", "/s/Roll 10/C.NEF", "/s/Roll 8/B.NEF"];
        let library: Vec<Entry> = paths.iter().map(|p| Entry::new(p, base, &format!("fp-{p}"))).collect();
        let client = Project::new("Client", base, at(&paths));
        let tops = roots(std::slice::from_ref(&client), &Reach::rows(&library));
        let open = flatten(&tops, &HashSet::from([NodeId::Project(client.id)]), 0);
        let shown: Vec<NodeId> = open[1..].iter().map(|(n, _)| n.id()).collect();
        let photo = |p: &str| NodeId::Photo { path: p.into(), project: client.id };
        assert_eq!(shown, paths.map(photo), "the project's order, the rolls interleaved");
        // Twin: grouped by roll, Roll 8's A would follow Roll 10's C.
        assert_ne!(shown[1], photo(paths[2]));
        // Twin: two copies of one file name stay two rows.
        assert_eq!(shown.iter().filter(|n| matches!(n, NodeId::Photo { path, .. } if path.ends_with("/A.NEF"))).count(), 2);
    }
}
