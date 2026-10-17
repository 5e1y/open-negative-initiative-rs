//! The sidebar's own tree, computed fresh from the groups the gallery cuts, never a second truth.

use crate::film_group::FilmGroup;
use crate::index::Entry;
use oni_settings::{Date, Id};
use std::collections::HashSet;

/// A day is its local midnight in whole seconds since 1970, as `FilmKey::Day` holds it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NodeId {
    Roll(Id),
    Day(i64),
    Photo(String),
    Folder(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Roll { id: Id, group: FilmGroup },
    Day { day: Date, group: FilmGroup },
    Photo(Entry),
    Folder { path: String, group: Option<FilmGroup>, children: Vec<Node> },
}

impl Node {
    pub fn id(&self) -> NodeId {
        match self {
            Node::Roll { id, .. } => NodeId::Roll(*id),
            Node::Day { day, .. } => NodeId::Day(day.unix_seconds() as i64),
            Node::Photo(entry) => NodeId::Photo(entry.path.clone()),
            Node::Folder { path, .. } => NodeId::Folder(path.clone()),
        }
    }

    pub fn entries(&self) -> Vec<Entry> {
        match self {
            Node::Roll { group, .. } | Node::Day { group, .. } => group.entries.clone(),
            Node::Photo(entry) => vec![entry.clone()],
            Node::Folder { group, children, .. } => {
                let mut out: Vec<Entry> = children.iter().flat_map(Node::entries).collect();
                out.extend(group.iter().flat_map(|g| g.entries.iter().cloned()));
                out
            }
        }
    }

    /// The photographs under here, summed rather than counted on `entries`, which copies them all.
    pub fn count(&self) -> usize {
        match self {
            Node::Roll { group, .. } | Node::Day { group, .. } => group.entries.len(),
            Node::Photo(_) => 1,
            Node::Folder { group, children, .. } => {
                children.iter().fold(group.as_ref().map_or(0, |g| g.entries.len()), |n, c| n + c.count())
            }
        }
    }
}

/// Laid flat with each node's depth: a roll or day opens into its photos, a folder into its
/// children then its own photos.
pub fn flatten(nodes: &[Node], expanded: &HashSet<NodeId>, depth: usize) -> Vec<(Node, usize)> {
    let mut rows = Vec::new();
    for node in nodes {
        rows.push((node.clone(), depth));
        if !expanded.contains(&node.id()) {
            continue;
        }
        match node {
            Node::Photo(_) => {}
            Node::Roll { group, .. } | Node::Day { group, .. } => {
                rows.extend(group.entries.iter().map(|e| (Node::Photo(e.clone()), depth + 1)));
            }
            Node::Folder { group, children, .. } => {
                rows.extend(flatten(children, expanded, depth + 1));
                rows.extend(group.iter().flat_map(|g| g.entries.iter().map(|e| (Node::Photo(e.clone()), depth + 1))));
            }
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::film_group::FilmKey;

    fn group(folder: &str, paths: &[&str]) -> FilmGroup {
        let base = Date::since_reference_date(8e8);
        FilmGroup {
            id: FilmKey::Folder(folder.into()),
            film: None,
            start: base,
            entries: paths
                .iter()
                .map(|p| {
                    let mut e = Entry::new(p, base, p);
                    e.captured_at = Some(base);
                    e
                })
                .collect(),
            heading: None,
        }
    }

    #[test]
    fn a_folder_opens_on_its_subfolders_then_its_photos() {
        let inner = Node::Folder {
            path: "/r/trip/day2".into(),
            group: Some(group("/r/trip/day2", &["/r/trip/day2/c"])),
            children: vec![],
        };
        let outer = Node::Folder {
            path: "/r/trip".into(),
            group: Some(group("/r/trip", &["/r/trip/a", "/r/trip/b"])),
            children: vec![inner],
        };
        let nodes = [outer.clone()];
        assert_eq!(flatten(&nodes, &HashSet::new(), 0).len(), 1);
        let open = flatten(&nodes, &HashSet::from([NodeId::Folder("/r/trip".into())]), 0);
        let ids: Vec<NodeId> = open.iter().map(|(n, _)| n.id()).collect();
        assert_eq!(
            ids,
            [
                NodeId::Folder("/r/trip".into()),
                NodeId::Folder("/r/trip/day2".into()),
                NodeId::Photo("/r/trip/a".into()),
                NodeId::Photo("/r/trip/b".into())
            ]
        );
        assert!(open[1..].iter().all(|(_, d)| *d == 1));
        let both = flatten(
            &nodes,
            &HashSet::from([NodeId::Folder("/r/trip".into()), NodeId::Folder("/r/trip/day2".into())]),
            0,
        );
        assert_eq!(both.len(), 5);
        assert_eq!(both.iter().find(|(n, _)| n.id() == NodeId::Photo("/r/trip/day2/c".into())).map(|r| r.1), Some(2));
        // Twin: a closed subfolder hides the photo it holds.
        assert!(!open.iter().any(|(n, _)| n.id() == NodeId::Photo("/r/trip/day2/c".into())));
        assert_eq!((outer.count(), outer.entries().len()), (3, 3));
    }
}
