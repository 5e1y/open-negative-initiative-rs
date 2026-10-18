//! `renameRequest` in the archive's column: the folder New Roll made is named in place once its row
//! shows beside the photo, committed through `renameFolder`; an empty or unchanged name keeps it.

use super::Ctl;
use crate::Library;
use oni_library::archive_tree::NodeId;
use oni_library::paths;
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
                c.state.borrow_mut().tree_renaming = None;
                c.publish_tree();
            }
        });
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
        self.publish_tree();
        if let Some(name) = named(draft, &paths::name(&path)) {
            self.rename_folder(&path, name);
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
