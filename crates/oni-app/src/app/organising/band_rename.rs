//! `GalleryEngine`'s renaming: a band's heading double-clicked is typed over in place; Return, a
//! click on the grid or the focus leaving commits through `renameGroup` or `renameProject`.

use super::*;

impl State {
    /// The band laid at `index`: the layout drops empty groups, so its rank skips them too.
    fn band_at(&self, index: usize) -> Option<&Band> {
        self.org.bands.iter().filter(|b| !b.entries.is_empty()).nth(index)
    }

    /// Whether a heading is typed over, which every scroll then lays again.
    pub(in crate::app) fn renaming_heading(&self) -> bool {
        self.org.renaming.is_some()
    }

    /// Whether the band laid at `index` is the one typed over.
    pub(in crate::app) fn heading_renamed(&self, index: usize) -> bool {
        let Some((naming, _)) = &self.org.renaming else { return false };
        self.band_at(index).and_then(|b| b.naming.as_ref()) == Some(naming)
    }

    /// The heading under a point of the document, across the grid's width: the pinned one first,
    /// as it lies over the band passing under it.
    pub(super) fn heading_at(&self, y: f32) -> Option<usize> {
        let header = self.layout.metrics.header;
        if let Some((i, top)) = self.layout.pinned(self.scroll)
            && (top..top + header).contains(&y)
        {
            return Some(i);
        }
        self.layout.bands.iter().position(|b| (b.top..b.top + header).contains(&y))
    }
}

impl Ctl {
    pub(super) fn wire_band_rename(me: &Rc<Ctl>, lib: &Library) {
        let p = me.clone();
        lib.on_heading_edited(move |t| {
            if let Some((_, draft)) = p.state.borrow_mut().org.renaming.as_mut() {
                *draft = t.to_string();
            }
            // A field laid again, its heading having moved, opens on what was typed.
            if let Some(w) = p.window() {
                w.global::<Library>().set_heading_draft(t);
            }
        });
        let p = me.clone();
        lib.on_heading_committed(move |t| p.commit_heading(&t));
        let p = me.clone();
        lib.on_heading_cancelled(move || {
            p.state.borrow_mut().org.renaming = None;
            p.publish_tiles();
        });
    }

    /// A release on a heading picks no cell; the second within the system's interval renames.
    pub(super) fn heading_clicked(self: &Rc<Self>, index: usize) {
        let second = {
            let mut s = self.state.borrow_mut();
            let now = Instant::now();
            let second = s.org.heading_click.is_some_and(|(i, at)| i == index && now - at < system::double_click());
            s.org.heading_click = if second { None } else { Some((index, now)) };
            second
        };
        if second {
            self.rename_heading(index);
        }
    }

    /// `rename(_:)`: another heading typed over is committed first; the field opens on the
    /// band's explicit name.
    fn rename_heading(self: &Rc<Self>, index: usize) {
        let Some((naming, explicit)) = self.state.borrow().band_at(index).and_then(|b| Some((b.naming.clone()?, b.explicit.clone()))) else {
            return;
        };
        if self.state.borrow().org.renaming.as_ref().is_some_and(|(n, _)| *n != naming) {
            self.exit_heading();
        }
        if let Some(w) = self.window() {
            w.global::<Library>().set_heading_draft(explicit.clone().into());
        }
        self.state.borrow_mut().org.renaming = Some((naming, explicit));
        self.publish_tiles();
    }

    /// `exitEditing()`: what the field holds is committed.
    pub(super) fn exit_heading(self: &Rc<Self>) {
        let draft = self.state.borrow().org.renaming.as_ref().map(|(_, d)| d.clone());
        if let Some(draft) = draft {
            self.commit_heading(&draft);
        }
    }

    /// `commit(_:)`: the field closes; a name trimmed, not empty and not the explicit one renames
    /// the folder or the project, and a band gone meanwhile renames nothing.
    fn commit_heading(self: &Rc<Self>, draft: &str) {
        let Some((naming, _)) = self.state.borrow_mut().org.renaming.take() else { return };
        let explicit = self.state.borrow().org.bands.iter().find(|b| b.naming.as_ref() == Some(&naming)).map(|b| b.explicit.clone());
        self.publish_tiles();
        let Some(name) = explicit.and_then(|was| picking::named(draft, &was)) else { return };
        match naming {
            Naming::Folder(path) => self.rename_folder(&path, &name),
            Naming::Project(id) => {
                let renamed = project::rename(&mut self.state.borrow_mut().org.projects, id, &name);
                if renamed {
                    self.save_projects();
                    self.lay_grid();
                }
            }
        }
    }
}
