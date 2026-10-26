//! The sidebar's own tree, computed fresh from the groups the gallery cuts, never a second truth.

use crate::film_group::FilmGroup;
use crate::index::Entry;
use std::collections::HashSet;

/// The archive is folders and the photos in them, as the disk lays them out.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NodeId {
    Photo(String),
    Folder(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Photo(Entry),
    Folder { path: String, group: Option<FilmGroup>, children: Vec<Node> },
}

impl Node {
    pub fn id(&self) -> NodeId {
        match self {
            Node::Photo(entry) => NodeId::Photo(entry.path.clone()),
            Node::Folder { path, .. } => NodeId::Folder(path.clone()),
        }
    }

    pub fn entries(&self) -> Vec<Entry> {
        match self {
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
            Node::Photo(_) => 1,
            Node::Folder { group, children, .. } => {
                children.iter().fold(group.as_ref().map_or(0, |g| g.entries.len()), |n, c| n + c.count())
            }
        }
    }
}

/// The folder node `id` names, searched through every folder; a photo in a folder has none.
pub fn find<'a>(nodes: &'a [Node], id: &NodeId) -> Option<&'a Node> {
    nodes.iter().find_map(|node| match node {
        _ if node.id() == *id => Some(node),
        Node::Folder { children, .. } => find(children, id),
        _ => None,
    })
}

/// `firstPath`: a photo itself, a folder's own first photo before its children's.
pub fn first_path(node: &Node) -> Option<&str> {
    match node {
        Node::Photo(entry) => Some(&entry.path),
        Node::Folder { group, children, .. } => {
            group.as_ref().and_then(|g| g.entries.first()).map(|e| e.path.as_str()).or_else(|| children.iter().find_map(first_path))
        }
    }
}

/// One row of the tree laid flat, borrowed from the tree it lies in.
#[derive(Debug, Clone, Copy)]
pub enum Laid<'a> {
    Folder(&'a Node),
    Photo(&'a Entry),
}

/// `flatten`'s rows in its order, each lent to `row` with its depth rather than copied with its subtree.
pub fn walk<'a>(nodes: &'a [Node], expanded: &HashSet<NodeId>, depth: usize, row: &mut dyn FnMut(Laid<'a>, usize)) {
    for node in nodes {
        match node {
            Node::Photo(entry) => row(Laid::Photo(entry), depth),
            Node::Folder { path, group, children } => {
                row(Laid::Folder(node), depth);
                if !expanded.contains(&NodeId::Folder(path.clone())) {
                    continue;
                }
                walk(children, expanded, depth + 1, row);
                for entry in group.iter().flat_map(|g| &g.entries) {
                    row(Laid::Photo(entry), depth + 1);
                }
            }
        }
    }
}

/// Laid flat with each node's depth: a folder opens into its children then its own photos.
pub fn flatten(nodes: &[Node], expanded: &HashSet<NodeId>, depth: usize) -> Vec<(Node, usize)> {
    let mut rows = Vec::new();
    for node in nodes {
        rows.push((node.clone(), depth));
        if !expanded.contains(&node.id()) {
            continue;
        }
        match node {
            Node::Photo(_) => {}
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
    use oni_settings::Date;

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
        // The walk lends the rows `flatten` copies, in its order and at its depths.
        let lent = |expanded: &HashSet<NodeId>| {
            let mut lent = Vec::new();
            walk(&nodes, expanded, 0, &mut |row, depth| {
                lent.push((match row { Laid::Folder(n) => n.id(), Laid::Photo(e) => NodeId::Photo(e.path.clone()) }, depth))
            });
            lent
        };
        let open = HashSet::from([NodeId::Folder("/r/trip".into()), NodeId::Folder("/r/trip/day2".into())]);
        for expanded in [HashSet::new(), open.clone()] {
            let copied: Vec<(NodeId, usize)> = flatten(&nodes, &expanded, 0).iter().map(|(n, d)| (n.id(), *d)).collect();
            assert_eq!(lent(&expanded), copied);
        }
        // Twin: the walk sees a fold, so the comparison above is not of two empty lists.
        assert_ne!(lent(&HashSet::new()), lent(&open));
    }

    #[test]
    fn a_folder_s_first_photo_is_its_own_before_its_subfolders() {
        let inner = Node::Folder { path: "/r/trip/day2".into(), group: Some(group("/r/trip/day2", &["/r/trip/day2/c"])), children: vec![] };
        let outer = Node::Folder { path: "/r/trip".into(), group: Some(group("/r/trip", &["/r/trip/a"])), children: vec![inner.clone()] };
        assert_eq!(first_path(&outer), Some("/r/trip/a"));
        let bare = Node::Folder { path: "/r/trip".into(), group: None, children: vec![inner] };
        assert_eq!(first_path(&bare), Some("/r/trip/day2/c"), "a folder of folders opens its first child's photo");
        let nodes = [outer];
        let found = find(&nodes, &NodeId::Folder("/r/trip/day2".into())).map(Node::id);
        assert_eq!(found, Some(NodeId::Folder("/r/trip/day2".into())), "a nested folder is found");
        assert!(find(&nodes, &NodeId::Folder("/r/elsewhere".into())).is_none(), "twin: a folder absent is not");
    }
}
