//! The gallery's pointer: a click, a drag that reorders a project, and the ground.

use super::*;

impl Ctl {
    pub(in crate::app) fn pointer_down(self: &Rc<Self>, x: f32, y: f32) {
        let mut s = self.state.borrow_mut();
        s.org.press = Some((x, y));
        s.org.reorder = None;
    }

    /// Only a project rearranges its photos by drag; the archive and the ONI Trash move nothing.
    pub(in crate::app) fn pointer_moved(self: &Rc<Self>, x: f32, y: f32) {
        {
            let mut s = self.state.borrow_mut();
            let Some((px, py)) = s.org.press else { return };
            if !s.org.projects_surface {
                return;
            }
            if s.org.reorder.is_none() {
                if (x - px).hypot(y - py) < DRAG_DISTANCE {
                    return;
                }
                let Some(start) = s.layout.index_at(px, py) else { return };
                let Some(lead) = s.order.get(start).cloned() else { return };
                let sources = if s.picks.selection.contains(&lead) { s.picks.selection.clone() } else { HashSet::from([lead.clone()]) };
                let width = s.layout.frame(start).map_or(0.0, |f| f.width);
                let size = (width, s.layout.tile.1);
                s.org.reorder = Some(Reorder { sources, lead, size, boundary: Some(start), at: (x, y) });
            }
            let boundary = s.layout.boundary(x, y);
            if let Some(r) = s.org.reorder.as_mut() {
                r.at = (x, y);
                if boundary.is_some() {
                    r.boundary = boundary;
                }
            }
        }
        self.publish_reorder();
    }

    pub(in crate::app) fn pointer_up(self: &Rc<Self>, x: f32, y: f32, command: bool, shift: bool) {
        let (press, reorder) = {
            let mut s = self.state.borrow_mut();
            (s.org.press.take(), s.org.reorder.take())
        };
        if let Some(r) = reorder {
            self.publish_reorder();
            return self.commit_reorder(r);
        }
        let Some((px, py)) = press else { return };
        // The document is as tall as its cells: a click under the last row reaches no gesture.
        if (x - px).hypot(y - py) >= DRAG_DISTANCE || y >= self.state.borrow().layout.height {
            return;
        }
        // `RollHeaderContent` lies over the document: a release on it reaches no cell.
        let heading = self.state.borrow().heading_at(y);
        if let Some(i) = heading {
            return self.heading_clicked(i);
        }
        self.exit_heading();
        let index = self.state.borrow().layout.index_at(x, y);
        match index {
            Some(i) => self.click(i, command, shift),
            None => self.click_empty(),
        }
    }

    pub(super) fn publish_reorder(&self) {
        let Some(w) = self.window() else { return };
        let lib = w.global::<Library>();
        let s = self.state.borrow();
        let Some(r) = &s.org.reorder else {
            lib.set_ghost_shown(false);
            lib.set_drop_shown(false);
            return;
        };
        let picture = s.fingerprints.get(&r.lead).and_then(|f| s.thumbs.get(f)).unwrap_or_default();
        lib.set_ghost(picture);
        lib.set_ghost_x(r.at.0);
        lib.set_ghost_y(r.at.1);
        lib.set_ghost_width(r.size.0);
        lib.set_ghost_height(r.size.1);
        lib.set_ghost_shown(true);
        match r.boundary.and_then(|b| s.layout.boundary_line(b, DROP_LINE)) {
            Some(line) => {
                lib.set_drop_x(line.x);
                lib.set_drop_y(line.y);
                lib.set_drop_width(line.width);
                lib.set_drop_height(line.height);
                lib.set_drop_shown(true);
            }
            None => lib.set_drop_shown(false),
        }
    }

    /// `commitReorder`: the boundary past the last cell reads back the last one.
    pub(super) fn commit_reorder(self: &Rc<Self>, r: Reorder) {
        let moved = {
            let mut s = self.state.borrow_mut();
            let Some(boundary) = r.boundary else { return };
            if s.order.is_empty() {
                return;
            }
            let at = boundary.min(s.order.len() - 1);
            let Some(target) = s.entry(&s.order[at].clone()).cloned() else { return };
            let rows = s.reach().into_owned();
            project::reorder_drop(&mut s.org.projects, &rows, &r.sources, &target)
        };
        if moved {
            self.save_projects();
            self.lay_grid();
        }
    }

    /// `click(_:)` on a cell: the picks, then the second plain click within the system's interval opens.
    pub(in crate::app) fn click(self: &Rc<Self>, index: usize, command: bool, shift: bool) {
        let (outcome, trash) = {
            let mut s = self.state.borrow_mut();
            let keys = s.band_keys();
            let order = s.order.clone();
            let columns = s.layout.columns;
            let shown = Shown { order: &order, groups: &keys, columns };
            let interval = system::double_click().as_secs_f64();
            let trash = s.trash_shown();
            if !trash {
                s.org.project_ring = None;
            }
            (s.shown_picks_mut().click(index, &shown, command, shift, uptime(), interval), trash)
        };
        match outcome {
            // The ONI Trash laid as a folder is; a tile opens nothing.
            Outcome::Open(path) if !trash => self.open_in_editor(&path),
            _ => {}
        }
        self.publish_tiles();
        self.publish_organising();
        self.publish_bar();
    }

    /// `onClickEmpty`: the ground clears the selection and the ring; the ONI Trash's, its selection.
    pub(super) fn click_empty(self: &Rc<Self>) {
        {
            let mut s = self.state.borrow_mut();
            if !s.trash_shown() {
                s.picks.anchor = None;
            }
            s.shown_picks_mut().clear();
            s.org.project_ring = None;
        }
        self.publish_tiles();
        self.publish_organising();
        self.publish_bar();
    }
}
