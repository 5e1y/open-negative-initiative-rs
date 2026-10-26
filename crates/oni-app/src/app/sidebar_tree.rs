//! The archive's tree beside a photo: the scroll that brings the photo it follows back to its
//! middle, the column's size, and `SidebarLayout`'s key.

use super::*;
use super::tree_column::Which;

impl Ctl {
    /// The photo the column follows: the open one, else the first picked.
    pub(super) fn tree_target(&self) -> Option<String> {
        let s = self.state.borrow();
        s.open.clone().or_else(|| s.picks.anchor.clone())
    }

    /// `TreeEngine.reveal`: the column centred on the photo it follows.
    pub(super) fn centre_tree(&self) {
        if self.unfold_archive() {
            self.publish_tree();
        }
        let target = self.tree_target();
        let row = {
            let mut s = self.state.borrow_mut();
            s.tree_centred = target.clone();
            target.and_then(|t| s.tree.paths.iter().position(|p| p.as_deref() == Some(t.as_str())))
        };
        if let Some(i) = row {
            self.centre_row(Which::Archive, i);
        }
    }

    /// Centres the column once the photo it follows changes, never on an edit, so a scroll by
    /// the wheel stays while the photo is worked on.
    pub(super) fn follow_tree(&self) {
        if self.state.borrow().tree_centred != self.tree_target() {
            self.centre_tree();
        }
    }

    pub(super) fn wire_tree(self: &Rc<Self>, lib: &Library) {
        let me = Rc::downgrade(self);
        lib.on_tree_resized(move |width, height| {
            let Some(c) = me.upgrade() else { return };
            let was = std::mem::replace(&mut c.state.borrow_mut().tree.size, (width, height));
            if was == (width, height) {
                return;
            }
            // A column first given its size centres on the photo; a resize keeps the scroll.
            if was.0 <= 0.0 || was.1 <= 0.0 {
                c.state.borrow_mut().tree_centred = None;
            }
            if was.0 != width {
                c.relay_column(Which::Archive);
            }
            c.clamp_column(Which::Archive);
            c.follow_tree();
            c.pin_column(Which::Archive);
            // A taller column brings more rows near.
            c.fill_tree_thumbnails(None);
        });
        // The column shown again, beside a photo or back on the archive: centred on the photo.
        let me = Rc::downgrade(self);
        lib.on_tree_shown(move |width, height| {
            let Some(c) = me.upgrade() else { return };
            let was = std::mem::replace(&mut c.state.borrow_mut().tree.size, (width, height));
            if was.0 != width {
                c.relay_column(Which::Archive);
            }
            c.centre_tree();
            c.pin_column(Which::Archive);
            c.fill_tree_thumbnails(None);
        });
        let me = Rc::downgrade(self);
        lib.on_tree_layout_next(move || {
            let Some(c) = me.upgrade() else { return };
            c.next_layout();
            c.publish_tree();
            c.publish_projects();
            // A new layout moves every row: the selection comes back to the middle.
            if c.state.borrow().org.projects_surface {
                c.project_centre_key();
            } else {
                c.centre_key();
            }
        });
        let me = Rc::downgrade(self);
        lib.on_tree_centre(move || {
            if let Some(c) = me.upgrade() {
                c.centre_key();
            }
        });
        let me = Rc::downgrade(self);
        lib.on_tree_reload(move || {
            if let Some(c) = me.upgrade() {
                c.act(crate::Act::Refresh);
            }
        });
        self.wire_tree_column(lib);
    }
}
