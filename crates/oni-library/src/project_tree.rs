//! The projects' sidebar tree: a project opens into its own photos, one level down.

use crate::index::Entry;
use crate::project::{self, Project, ProjectGroup};
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

    pub fn entries(&self) -> Vec<Entry> {
        match self {
            Node::Project(group) => group.entries.clone(),
            Node::Photo { entry, .. } => vec![entry.clone()],
        }
    }
}

/// Every project, newest first; a photo in no project is absent.
pub fn roots(projects: &[Project], entries: &[Entry]) -> Vec<Node> {
    project::grouped(projects, entries).into_iter().map(Node::Project).collect()
}

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
    use oni_settings::Date;

    #[test]
    fn a_project_opens_into_its_photos_each_tagged_with_its_project() {
        let base = Date::since_reference_date(8e8);
        let library = [Entry::new("/negs/a.RW2", base, "fp-a"), Entry::new("/negs/b.RW2", base, "fp-b")];
        let older = Project::new("Older", base, vec!["fp-a".into()]);
        let newer = Project::new("Newer", crate::later(base, 3600.0), vec!["fp-b".into()]);
        let tops = roots(&[older, newer.clone()], &library);
        assert_eq!(tops.len(), 2);
        assert_eq!(tops[0].id(), NodeId::Project(newer.id));
        assert_eq!(flatten(&tops, &HashSet::new(), 0).len(), 2);
        let open = flatten(&tops, &HashSet::from([NodeId::Project(newer.id)]), 0);
        assert_eq!(open.len(), 3);
        assert_eq!(open[1].0.id(), NodeId::Photo { path: "/negs/b.RW2".into(), project: newer.id });
        assert_eq!(open[1].1, 1);

        let a = Project::new("Shared A", base, vec!["fp-a".into()]);
        let b = Project::new("Shared B", crate::later(base, 60.0), vec!["fp-a".into()]);
        let both = flatten(
            &roots(&[a.clone(), b.clone()], &library),
            &HashSet::from([NodeId::Project(a.id), NodeId::Project(b.id)]),
            0,
        );
        let rows: Vec<NodeId> =
            both.iter().filter(|(n, _)| matches!(n, Node::Photo { .. })).map(|(n, _)| n.id()).collect();
        assert_eq!(rows.len(), 2);
        // Twin: the two rows of one photo carry distinct ids, one per project.
        assert_ne!(rows[0], rows[1]);
        let gap = Project::new("Gap", base, vec!["fp-a".into(), "fp-missing".into()]);
        assert_eq!(
            flatten(&roots(std::slice::from_ref(&gap), &library), &HashSet::from([NodeId::Project(gap.id)]), 0).len(),
            2
        );
    }
}
