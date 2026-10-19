//! `revealInTree`: the library column unfolds whatever holds the photo it follows, on the archive
//! and on the projects, once each time that photo changes, so a fold made by hand afterwards stays.

use super::*;
use oni_library::project;
use oni_library::project_tree::NodeId as ProjectNode;

/// The photo each tree last unfolded for, and whether the next archive laid is a folder just opened.
#[derive(Default)]
pub(super) struct Reveal {
    archive: Option<String>,
    project: Option<String>,
    /// `openFolder` unfolds every folder of the folder opened, whatever was folded before.
    pub fresh: bool,
}

impl State {
    /// `anchoredHere`: a photo ringed by a click in the tree leaves the column where the hand is;
    /// the folders above it still unfold.
    pub(super) fn anchored_here(&mut self, path: &str) {
        self.tree_centred = Some(path.to_owned());
    }

    /// `anchoredHere` on the projects: the click set the ring and the fold itself.
    pub(super) fn project_anchored(&mut self, path: &str) {
        self.reveal.project = Some(path.to_owned());
    }

    /// The first project holding `path`, as `Project.grouped(...).first` finds it.
    fn project_of(&self, path: &str) -> Option<oni_settings::Id> {
        project::grouped(&self.org.projects, &self.rows).into_iter().find(|g| g.entries.iter().any(|e| e.path == path)).map(|g| g.id)
    }
}

impl Ctl {
    /// `expandAncestors` on the ring's move (`EditorScene+Stages.swift:121`): the folders above the
    /// photo the column follows unfold; answers whether a row was added.
    pub(super) fn unfold_archive(&self) -> bool {
        let target = self.tree_target();
        let mut s = self.state.borrow_mut();
        if s.reveal.archive == target {
            return false;
        }
        s.reveal.archive = target.clone();
        match (target, s.archive.as_mut()) {
            (Some(path), Some(archive)) => archive.unfold_to(&path),
            _ => false,
        }
    }

    /// `centreSelected` (`EditorScene+Removal.swift:300`): the key unfolds the photo's folders
    /// again, one a hand folded included, then centres it.
    pub(super) fn centre_key(&self) {
        self.state.borrow_mut().reveal.archive = None;
        self.centre_tree();
    }

    /// The ring's move on the projects (`EditorScene+Stages.swift:124`): the photo rings under the
    /// first project holding it, which unfolds, unless it is ringed already under one.
    pub(super) fn reveal_project(&self) {
        let found = {
            let mut s = self.state.borrow_mut();
            let target = s.picks.anchor.clone();
            if s.reveal.project == target {
                return;
            }
            s.reveal.project = target.clone();
            let Some(path) = target.filter(|_| s.org.projects_surface) else { return };
            if matches!(&s.org.project_ring, Some(ProjectNode::Photo { path: p, .. }) if *p == path) {
                return;
            }
            let Some(id) = s.project_of(&path) else { return };
            s.org.project_ring = Some(ProjectNode::Photo { path, project: id });
            s.org.project_ring.clone()
        };
        if let Some(node) = found {
            self.centre_project(&node);
        }
    }

    /// `centreSelected` on the projects: the ringed photo, else the open one, under the first
    /// project holding it.
    pub(super) fn project_centre_key(&self) {
        let node = {
            let s = self.state.borrow();
            let path = match &s.org.project_ring {
                Some(ProjectNode::Photo { path, .. }) => Some(path.clone()),
                _ => s.open.clone(),
            };
            path.and_then(|path| s.project_of(&path).map(|project| ProjectNode::Photo { path, project }))
        };
        if let Some(node) = node {
            self.centre_project(&node);
        }
    }

    /// `expandProjectAncestors` then `TreeEngine.reveal`: the project opens and the list centres the row.
    fn centre_project(&self, node: &ProjectNode) {
        let Some(w) = self.window() else { return };
        if let ProjectNode::Photo { project, .. } = node {
            self.state.borrow_mut().org.project_expanded.insert(ProjectNode::Project(*project));
        }
        self.publish_projects();
        let row = self.state.borrow().org.project_nodes.iter().position(|n| n == node);
        if let Some(row) = row {
            let lib = w.global::<Library>();
            lib.set_project_reveal_row(row as i32);
            lib.set_project_reveal_serial(lib.get_project_reveal_serial() + 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::library::Archive;
    use oni_library::archive_tree::NodeId;
    use oni_library::paths;
    use std::collections::HashSet;

    #[test]
    fn every_folder_opens_unfolded_and_a_photo_unfolds_only_the_folders_above_it() {
        let root = std::env::temp_dir().join(format!("oni-app-reveal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let scans = root.join("Scans");
        for (name, bytes) in [("A/a.NEF", &b"first"[..]), ("B/b.NEF", b"second"), ("B/C/c.NEF", b"third")] {
            let path = scans.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, bytes).unwrap();
        }
        let mut archive = Archive::load(&scans);
        let folder = |p: &str| NodeId::Folder(paths::canonical(&scans.join(p)));
        let every: HashSet<NodeId> = ["", "A", "B", "B/C"].iter().map(|p| folder(p)).collect();
        assert_eq!(archive.expanded, every, "the opened folder and every folder under it start unfolded");

        archive.expanded.clear();
        let c = paths::canonical(&scans.join("B/C/c.NEF"));
        let shown = |a: &Archive| a.rows().iter().any(|r| r.id == NodeId::Photo(c.clone()));
        assert!(!shown(&archive), "twin: a folded tree hides the photo");
        assert!(archive.unfold_to(&c));
        assert!(shown(&archive));
        let above: HashSet<NodeId> = ["", "B", "B/C"].iter().map(|p| folder(p)).collect();
        assert_eq!(archive.expanded, above, "a folder beside the photo's stays folded");
        assert!(!archive.unfold_to(&c), "twin: unfolded already, nothing is added");
        let _ = std::fs::remove_dir_all(&root);
    }
}
