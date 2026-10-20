//! `TreeEngine.rename` in the column's trees: a folder typed over commits through `renameFolder`,
//! a project through `renameProject`; the request New Roll or Rename… makes lands once its row
//! shows beside the photo. An empty or unchanged name keeps the old one.

use super::Ctl;
use crate::Library;
use oni_library::archive_tree::NodeId;
use oni_library::{paths, project};
use slint::ComponentHandle;
use std::rc::Rc;

impl Ctl {
    pub(super) fn wire_tree_rename(me: &Rc<Ctl>, window: &crate::MainWindow) {
        let lib = window.global::<Library>();
        let c = Rc::downgrade(me);
        lib.on_row_rename_committed(move |_, draft| {
            if let Some(c) = c.upgrade() {
                c.tree_rename_committed(&draft);
            }
        });
        let c = Rc::downgrade(me);
        lib.on_row_rename_cancelled(move || {
            if let Some(c) = c.upgrade() {
                let mut s = c.state.borrow_mut();
                s.tree_renaming = None;
                s.org.tree_draft = None;
                drop(s);
                c.publish_tree();
            }
        });
        let c = Rc::downgrade(me);
        lib.on_row_rename_edited(move |draft| {
            if let Some(c) = c.upgrade() {
                c.state.borrow_mut().org.tree_draft = Some(draft.to_string());
            }
        });
        let c = Rc::downgrade(me);
        lib.on_project_row_rename_committed(move |_, draft| {
            if let Some(c) = c.upgrade() {
                c.project_rename_committed(&draft);
            }
        });
        let c = Rc::downgrade(me);
        lib.on_project_row_rename_cancelled(move || {
            if let Some(c) = c.upgrade() {
                let mut s = c.state.borrow_mut();
                s.org.project_renaming = None;
                s.org.tree_draft = None;
                drop(s);
                c.publish_projects();
            }
        });
    }

    /// `renameRequest = .folder(folder)` from the tree's menu: the row typed over in place.
    pub(super) fn rename_in_tree(self: &Rc<Self>, folder: String) {
        self.exit_tree_editing();
        self.state.borrow_mut().rename_request = Some(folder);
        self.publish_tree();
    }

    /// `exitEditing()`: a click, a fold or another rename in a tree commits what the field holds.
    pub(super) fn exit_tree_editing(self: &Rc<Self>) {
        let (archive, project, draft) = {
            let s = self.state.borrow();
            (s.tree_renaming.is_some(), s.org.project_renaming.is_some(), s.org.tree_draft.clone())
        };
        // A field never typed in holds the name it opened on, which renames nothing.
        let draft = draft.unwrap_or_default();
        if archive {
            self.tree_rename_committed(&draft);
        }
        if project {
            self.project_rename_committed(&draft);
        }
    }

    /// The column beside the photo has just shown: a waiting request may land on its rows.
    pub(super) fn land_tree_request(&self) {
        if self.state.borrow().rename_request.is_some() {
            self.publish_tree();
        }
    }

    /// `commit`: `GalleryView.named` keeps the folder on an empty or unchanged name.
    fn tree_rename_committed(self: &Rc<Self>, draft: &str) {
        let Some(path) = self.state.borrow_mut().tree_renaming.take() else { return };
        self.state.borrow_mut().org.tree_draft = None;
        self.publish_tree();
        if let Some(name) = named(draft, &paths::name(&path)) {
            self.rename_folder(&path, name);
        }
    }

    /// `renameProject`: the name written to the projects' file; a project gone meanwhile keeps none.
    fn project_rename_committed(self: &Rc<Self>, draft: &str) {
        let was = {
            let mut s = self.state.borrow_mut();
            s.org.tree_draft = None;
            let Some(id) = s.org.project_renaming.take() else { return };
            s.org.projects.iter().find(|p| p.id == id).map(|p| (id, p.name.clone()))
        };
        self.publish_projects();
        let Some((id, name)) = was.and_then(|(id, was)| named(draft, &was).map(|n| (id, n.to_owned()))) else { return };
        if project::rename(&mut self.state.borrow_mut().org.projects, id, &name) {
            self.save_projects();
            self.lay_grid();
            self.publish_projects();
        }
    }
}

/// `GalleryView.named`: the draft trimmed, `None` when empty or the name it was.
pub fn named<'a>(draft: &'a str, was: &str) -> Option<&'a str> {
    let wanted = draft.trim();
    (!wanted.is_empty() && wanted != was).then_some(wanted)
}

/// `TreeEngine.requestLanded`: the request turns into the row's field once the column beside the
/// photo holds that folder's row; until then it waits, as Swift's binding does.
pub(super) fn land(request: &mut Option<String>, renaming: &mut Option<String>, shown: &[NodeId], beside_photo: bool) {
    let Some(wanted) = request.as_ref().filter(|_| beside_photo) else { return };
    if shown.iter().any(|id| matches!(id, NodeId::Folder(p) if p == wanted)) {
        *renaming = request.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rename_request_lands_only_on_its_rows_column_beside_the_photo() {
        let shown = [NodeId::Folder("/r/2026-10-18".into()), NodeId::Photo("/r/2026-10-18/A.tif".into())];
        let (mut request, mut renaming) = (Some("/r/2026-10-18".to_string()), None);
        land(&mut request, &mut renaming, &shown, false);
        assert_eq!((request.as_deref(), renaming.as_deref()), (Some("/r/2026-10-18"), None), "the gallery's column waits");
        land(&mut request, &mut renaming, &shown[1..], true);
        assert_eq!(renaming, None, "twin: a photo's row under it is not the folder's");
        land(&mut request, &mut renaming, &shown, true);
        assert_eq!((request, renaming.as_deref()), (None, Some("/r/2026-10-18")));
    }

    #[test]
    fn a_name_is_trimmed_and_an_empty_or_unchanged_one_keeps_the_folder() {
        assert_eq!(named("  Holiday \n", "2026-10-18"), Some("Holiday"));
        assert_eq!(named(" 2026-10-18 ", "2026-10-18"), None);
        assert_eq!(named(" \t", "2026-10-18"), None);
        assert_ne!(named("Holiday", "Holiday 2"), None, "twin: another name is a rename");
    }
}
