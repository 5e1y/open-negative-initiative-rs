//! A click in the library column's trees, as `TreeEngine.click` reads it: the chevron's zone folds,
//! the row rings its node and, beside a photo, opens the photo itself or the first one it holds.

use super::organising::Column;
use super::*;
use oni_library::archive_tree::{self, NodeId};
use oni_library::project;
use oni_library::project_tree::NodeId as ProjectNode;

impl Ctl {
    fn beside_photo(&self) -> bool {
        self.window().is_some_and(|w| !w.global::<App>().get_gallery())
    }

    /// `toggleExpand`: the chevron's zone folds or unfolds the row and nothing else.
    pub(super) fn row_folded(self: &Rc<Self>, i: usize) {
        self.exit_tree_editing();
        {
            let mut s = self.state.borrow_mut();
            let Some(archive) = s.archive.as_mut() else { return };
            let Some(row) = archive.rows().get(i).cloned() else { return };
            archive.toggle(&row.id);
        }
        self.publish_tree();
    }

    /// `click` then `openFromTree` (`EditorScene+Lab.swift:110`): a photo row opens its photo, a
    /// group's row beside a photo its first one; the column stays where the hand is. With ⌘ or ⇧
    /// the row joins the tree's pick, whose photos become the selection (`pickInColumn`).
    pub(super) fn row_clicked(self: &Rc<Self>, i: usize, command: bool, shift: bool) {
        self.exit_tree_editing();
        let picked = {
            let mut guard = self.state.borrow_mut();
            let s = &mut *guard;
            // A double click's second does a picking click's work again, as Swift's tracker lets it.
            if s.second_click(Column::Archive, i) && !command && !shift {
                return;
            }
            let Some(id) = s.org.tree_ids.get(i).cloned() else { return };
            if !s.org.tree_pick.click(&id, &s.org.tree_ids, command, shift) {
                false
            } else {
                s.picks.selection = crate::tree_pick::picked_photos(&s.org.tree_pick.selection, &s.survey.images);
                true
            }
        };
        if picked {
            self.publish_tiles();
            self.publish_organising();
            return self.publish_bar();
        }
        if self.state.borrow().org.trash_open {
            self.leave_trash();
            self.lay_grid();
        }
        let (path, photo) = {
            let s = self.state.borrow();
            let Some(archive) = s.archive.as_ref() else { return };
            let Some(row) = archive.rows().get(i).cloned() else { return };
            match row.id {
                NodeId::Photo(path) => (Some(path), true),
                id => (archive_tree::find(&archive.nodes, &id).and_then(archive_tree::first_path).map(str::to_owned), false),
            }
        };
        let Some(path) = path.filter(|_| photo || self.beside_photo()) else { return self.publish_tree() };
        if !self.take_off_roll() {
            return;
        }
        let reopened = self.state.borrow().open.as_deref() == Some(path.as_str());
        {
            let mut s = self.state.borrow_mut();
            s.designate(&path);
            s.anchored_here(&path);
        }
        if !reopened {
            self.open_photo(&path);
        }
        self.publish_tiles();
        self.publish_organising();
    }

    /// `takeOff` on a photo row's double click; on a folder's, `rename` types over its name.
    pub(super) fn row_double_clicked(self: &Rc<Self>, i: usize) {
        let id = self.state.borrow().archive.as_ref().and_then(|a| a.rows().get(i).map(|r| r.id.clone()));
        match id {
            Some(NodeId::Photo(path)) => self.open_in_editor(&path),
            Some(NodeId::Folder(path)) => {
                self.exit_tree_editing();
                self.state.borrow_mut().tree_renaming = Some(path);
                self.publish_tree();
            }
            _ => {}
        }
    }

    /// `clickEmpty` (`TreeEngine.swift:427`, `EditorScene+Removal.swift:581`): a click on no row
    /// drops the tree's pick, the selection and the ring; the photo open stays open.
    pub(super) fn tree_empty_clicked(self: &Rc<Self>) {
        self.exit_tree_editing();
        {
            let mut s = self.state.borrow_mut();
            s.org.tree_pick.clear();
            s.picks.selection.clear();
            s.picks.anchor = None;
        }
        self.publish_tiles();
        self.publish_organising();
        self.publish_bar();
    }

    pub(super) fn project_row_folded(self: &Rc<Self>, i: usize) {
        self.exit_tree_editing();
        {
            let mut s = self.state.borrow_mut();
            let Some(id) = s.org.project_nodes.get(i).cloned().filter(|id| matches!(id, ProjectNode::Project(_))) else { return };
            if !s.org.project_expanded.remove(&id) {
                s.org.project_expanded.insert(id);
            }
        }
        self.publish_projects();
    }

    /// `selectProject`, then `openFromTree(project:)` beside a photo: a project row rings it and
    /// clears the photos, a photo row selects it; beside a photo either opens its photo. With ⌘
    /// or ⇧ the row joins the tree's pick, whose photos become the selection.
    pub(super) fn project_row_clicked(self: &Rc<Self>, i: usize, command: bool, shift: bool) {
        // A press that became a drag lets go on its row: the drop, never a click.
        if self.project_dragging() {
            return;
        }
        self.exit_tree_editing();
        let picked = {
            let mut guard = self.state.borrow_mut();
            let s = &mut *guard;
            if s.second_click(Column::Projects, i) && !command && !shift {
                return;
            }
            let Some(id) = s.org.project_nodes.get(i).cloned() else { return };
            if !s.org.project_pick.click(&id, &s.org.project_nodes, command, shift) {
                false
            } else {
                let pick = &s.org.project_pick.selection;
                s.picks.selection = pick.iter().filter_map(|n| if let ProjectNode::Photo { path, .. } = n { Some(path.clone()) } else { None }).collect();
                true
            }
        };
        if picked {
            self.publish_tiles();
            self.publish_organising();
            return self.publish_bar();
        }
        let beside = self.beside_photo();
        let open = {
            let mut guard = self.state.borrow_mut();
            let s = &mut *guard;
            let Some(id) = s.org.project_nodes.get(i).cloned() else { return };
            s.org.project_ring = Some(id.clone());
            let path = match &id {
                ProjectNode::Project(project) => {
                    s.picks.selection.clear();
                    s.picks.cursor = None;
                    let groups = project::grouped(&s.org.projects, &s.rows);
                    let first = groups.iter().find(|g| g.id == *project).and_then(|g| g.entries.first()).map(|e| e.path.clone());
                    // `revealInTree`: the photo opened rings under its project, which opens.
                    first.filter(|_| beside).inspect(|path| {
                        s.org.project_expanded.insert(id.clone());
                        s.org.project_ring = Some(ProjectNode::Photo { path: path.clone(), project: *project });
                    })
                }
                ProjectNode::Photo { path, .. } => {
                    s.picks.selection = HashSet::from([path.clone()]);
                    s.picks.cursor = Some(path.clone());
                    s.picks.anchor = Some(path.clone());
                    beside.then(|| path.clone())
                }
            };
            if let Some(path) = &path {
                s.designate(path);
                s.project_anchored(path);
            }
            path.filter(|p| s.open.as_deref() != Some(p.as_str()))
        };
        if let Some(path) = open {
            self.open_photo(&path);
        }
        self.publish_tiles();
        self.publish_organising();
        self.publish_bar();
    }

    /// `takeOff` on a photo row's double click; on a project's, `rename` types over its name.
    pub(super) fn project_row_double_clicked(self: &Rc<Self>, i: usize) {
        if self.project_dragging() {
            return;
        }
        let id = self.state.borrow().org.project_nodes.get(i).cloned();
        match id {
            Some(ProjectNode::Photo { path, .. }) => self.open_in_editor(&path),
            Some(ProjectNode::Project(project)) => {
                self.exit_tree_editing();
                self.state.borrow_mut().org.project_renaming = Some(project);
                self.publish_projects();
            }
            None => {}
        }
    }
}
