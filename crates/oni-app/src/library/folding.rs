//! The tree's folding: which folders stand open, unfolded to a photo, every one, or as a hand left them.

use super::*;

impl Archive {
    /// `expandAncestors`: every folder above `path` up to the opened one unfolds; answers whether
    /// a row was added.
    pub fn unfold_to(&mut self, path: &str) -> bool {
        let mut added = false;
        let mut folder = paths::parent(path);
        while folder != self.root && paths::holds(&self.root, &folder) {
            added |= self.expanded.insert(NodeId::Folder(folder.clone()));
            let up = paths::parent(&folder);
            if up == folder {
                break;
            }
            folder = up;
        }
        if added {
            self.laid.take();
        }
        added
    }

    /// Every folder unfolded, the opened one included, as a folder just opened lays them.
    pub fn unfold_every(&mut self) {
        let mut every = HashSet::new();
        folder_ids(&self.nodes, &mut every);
        self.set_expanded(every);
    }

    /// The nodes unfolded, folders at any depth.
    pub fn expanded(&self) -> &HashSet<NodeId> {
        &self.expanded
    }

    pub fn set_expanded(&mut self, expanded: HashSet<NodeId>) {
        if self.expanded != expanded {
            self.expanded = expanded;
            self.laid.take();
        }
    }

    /// One node unfolded more.
    pub fn unfold(&mut self, id: NodeId) {
        if self.expanded.insert(id) {
            self.laid.take();
        }
    }

    /// The nodes in `folded` folded again, as a hand left them.
    pub fn keep_folded(&mut self, folded: &HashSet<NodeId>) {
        let before = self.expanded.len();
        self.expanded.retain(|id| !folded.contains(id));
        if self.expanded.len() != before {
            self.laid.take();
        }
    }

    /// Every node a hand left folded, a folder at any depth included.
    pub fn folded(&self) -> HashSet<NodeId> {
        let mut folders = HashSet::new();
        folder_ids(&self.nodes, &mut folders);
        self.nodes.iter().map(Node::id).chain(folders).filter(|id| !self.expanded.contains(id)).collect()
    }

    /// Opens or closes a row's node; a photo row answers its path instead.
    pub fn toggle(&mut self, id: &NodeId) -> Option<String> {
        if let NodeId::Photo(path) = id {
            return Some(path.clone());
        }
        if !self.expanded.remove(id) {
            self.expanded.insert(id.clone());
        }
        self.laid.take();
        None
    }
}

pub(super) fn folder_ids(nodes: &[Node], out: &mut HashSet<NodeId>) {
    for node in nodes {
        if let Node::Folder { children, .. } = node {
            out.insert(node.id());
            folder_ids(children, out);
        }
    }
}
