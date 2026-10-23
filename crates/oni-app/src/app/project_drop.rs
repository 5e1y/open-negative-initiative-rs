//! A drag in the projects' tree (`TreeEngine.swift:234-244,458-552`, `EditorScene+Rolls.swift:202-271`):
//! the pick or the row pressed, onto a project to join it or between two rows, to `projects.json` alone.

use super::organising::DRAG_DISTANCE;
use super::tree_column::Which;
use super::*;
use crate::TreeDrag;
use crate::sidebar::{self, DropTarget};
use oni_library::project;
use oni_library::project_tree::NodeId;

/// A drag under way: the rows it carries, the pointer in the tree's coordinates, where it would land.
#[derive(Clone, Debug)]
pub(super) struct Drag {
    items: HashSet<NodeId>,
    at: (f32, f32),
    target: Option<DropTarget>,
}

/// `addToProjectDrop`: photos dropped onto a project join it, and leave every other.
fn join(projects: &mut Vec<oni_library::project::Project>, rows: &[oni_library::Entry], items: &HashSet<NodeId>, target: &NodeId) -> bool {
    let NodeId::Project(id) = target else { return false };
    let paths: HashSet<String> = items.iter().filter_map(|n| if let NodeId::Photo { path, .. } = n { Some(path.clone()) } else { None }).collect();
    project::add_to(projects, rows, &paths, *id)
}

/// `reorderProjectTreeDrop`: before a photo the photos land in its project; before a project the
/// projects take their places among the others.
fn reorder(projects: &mut Vec<oni_library::project::Project>, rows: &[oni_library::Entry], items: &HashSet<NodeId>, target: &NodeId) -> bool {
    match target {
        NodeId::Photo { path, .. } => {
            let Some(entry) = rows.iter().find(|e| e.path == *path) else { return false };
            let paths: HashSet<String> = items.iter().filter_map(|n| if let NodeId::Photo { path, .. } = n { Some(path.clone()) } else { None }).collect();
            !paths.is_empty() && project::reorder_drop(projects, rows, &paths, entry)
        }
        NodeId::Project(id) => {
            let ids: HashSet<oni_settings::Id> = items.iter().filter_map(|n| if let NodeId::Project(p) = n { Some(*p) } else { None }).collect();
            !ids.is_empty() && project::reorder_root(projects, rows, &ids, *id)
        }
    }
}

impl Ctl {
    /// A press on row `i` moved from `from` to `at`: past the drag distance it carries the pick when
    /// the row is in it, else the row (`dragged()`); each move resolves where it would land.
    pub(super) fn project_row_dragged(&self, i: usize, from: (f32, f32), at: (f32, f32)) {
        let Some(w) = self.window() else { return };
        let strip = w.global::<crate::DS>().get_space_md() / 2.0;
        {
            let mut guard = self.state.borrow_mut();
            let s = &mut *guard;
            if s.org.project_drag.is_none() {
                if (at.0 - from.0).hypot(at.1 - from.1) < DRAG_DISTANCE {
                    return;
                }
                let Some(id) = s.org.project_nodes.get(i).cloned() else { return };
                let pick = &s.org.project_pick.selection;
                let items = if pick.contains(&id) { pick.clone() } else { HashSet::from([id]) };
                s.org.project_drag = Some(Drag { items, at, target: None });
            }
            let l = &s.project_tree;
            let target = sidebar::drop_target(at.0, at.1, &l.frames, &l.photos, &l.depths, true, strip);
            if let Some(d) = s.org.project_drag.as_mut() {
                (d.at, d.target) = (at, target);
            }
        }
        self.publish_project_drag();
    }

    /// The press let go: `performDrop` on where the drag would land; a cancel drops nothing.
    pub(super) fn project_drag_ended(self: &Rc<Self>, dropped: bool) {
        let Some(drag) = self.state.borrow_mut().org.project_drag.take() else { return };
        self.publish_project_drag();
        let moved = {
            let mut guard = self.state.borrow_mut();
            let s = &mut *guard;
            let rows = &s.rows;
            let node = |i: usize| s.org.project_nodes.get(i).cloned();
            match drag.target.filter(|_| dropped) {
                Some(DropTarget::Join(i)) => node(i).is_some_and(|id| !drag.items.contains(&id) && join(&mut s.org.projects, rows, &drag.items, &id)),
                Some(DropTarget::Before(i)) => node(i).is_some_and(|id| reorder(&mut s.org.projects, rows, &drag.items, &id)),
                None => false,
            }
        };
        if moved {
            self.save_projects();
            self.lay_grid();
        }
    }

    /// Whether a press on the projects' tree became a drag, whose release is no click.
    pub(super) fn project_dragging(&self) -> bool {
        self.state.borrow().org.project_drag.is_some()
    }

    /// `TreeDrop`'s hover: the ring round the row joined, or the line before the row landed before,
    /// across a name row or beside a thumbnail; the chip names how many rows ride.
    fn publish_project_drag(&self) {
        let Some(w) = self.window() else { return };
        let ds = w.global::<crate::DS>();
        let (emphasis, small) = (ds.get_stroke_emphasis(), ds.get_space_sm());
        let s = self.state.borrow();
        let mark = s.org.project_drag.as_ref().map_or_else(TreeDrag::default, |d| {
            let n = d.items.len();
            let chip = trn("{n} item", "{n} items", n as u64);
            let mut mark = TreeDrag { shown: true, chip: chip.into(), x: d.at.0, y: d.at.1, ..TreeDrag::default() };
            let l = s.laid(Which::Projects);
            let (i, ring) = match d.target {
                Some(DropTarget::Join(i)) => (i, true),
                Some(DropTarget::Before(i)) => (i, false),
                None => return mark,
            };
            let Some(f) = l.frames.get(i).copied() else { return mark };
            let (x, y, width, height) = if ring {
                (f.x, f.y, f.width, f.height)
            } else if l.photos.get(i).copied().unwrap_or(false) {
                (f.x - small / 2.0 - emphasis / 2.0, f.y, emphasis, f.height)
            } else {
                (0.0, f.y - emphasis / 2.0, l.size.0, emphasis)
            };
            (mark.ring, mark.line) = (ring, !ring);
            (mark.mark_x, mark.mark_y, mark.mark_width, mark.mark_height) = (x, y, width, height);
            mark
        });
        drop(s);
        w.global::<Library>().set_project_drag(mark);
    }

    /// `clickEmpty` then `clickEmptyProjects` (`TreeEngine.swift:427`, `EditorScene+Rolls.swift:300`):
    /// a click on no row drops the tree's pick, the selection and the project's ring.
    pub(super) fn project_empty_clicked(self: &Rc<Self>) {
        self.exit_tree_editing();
        {
            let mut s = self.state.borrow_mut();
            s.org.project_pick.clear();
            s.picks.selection.clear();
            s.org.project_ring = None;
        }
        self.publish_tiles();
        self.publish_organising();
        self.publish_bar();
    }

    pub(super) fn wire_project_drop(self: &Rc<Self>, lib: &Library) {
        let me = Rc::downgrade(self);
        lib.on_project_row_dragged(move |i, px, py, x, y| {
            if let Some(c) = me.upgrade() {
                c.project_row_dragged(i.max(0) as usize, (px, py), (x, y));
            }
        });
        let me = Rc::downgrade(self);
        lib.on_project_drag_ended(move |dropped| {
            if let Some(c) = me.upgrade() {
                c.project_drag_ended(dropped);
            }
        });
        let me = Rc::downgrade(self);
        lib.on_project_empty_clicked(move || {
            if let Some(c) = me.upgrade() {
                c.project_empty_clicked();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_library::project::Project;
    use oni_settings::Date;

    fn entry(path: &str) -> oni_library::Entry {
        oni_library::Entry::new(path, Date::since_reference_date(8e8), &format!("f-{path}"))
    }

    #[test]
    fn a_photo_dropped_on_a_project_joins_it_and_before_a_photo_lands_there() {
        let rows: Vec<_> = ["a", "b", "c"].iter().map(|p| entry(p)).collect();
        let at = |m: f64| Date::since_reference_date(8e8 + m);
        let mut projects = vec![Project::new("One", at(2.0), vec!["f-a".into(), "f-b".into()]), Project::new("Two", at(1.0), vec!["f-c".into()])];
        let (one, two) = (projects[0].id, projects[1].id);
        let photo = |p: &str, project| NodeId::Photo { path: p.into(), project };
        let c = HashSet::from([photo("c", two)]);
        // Twin: a photo dropped onto a photo joins nothing; only a project takes a member.
        assert!(!join(&mut projects, &rows, &c, &photo("a", one)));
        assert!(join(&mut projects, &rows, &c, &NodeId::Project(one)));
        assert_eq!(projects.len(), 1, "Two, emptied, goes");
        assert_eq!(projects[0].members, ["f-a", "f-b", "f-c"]);
        assert!(reorder(&mut projects, &rows, &c, &photo("a", one)));
        assert_eq!(projects[0].members, ["f-c", "f-a", "f-b"], "before a photo, in its place");
        // Twin: a project dropped before a photo moves nothing.
        assert!(!reorder(&mut projects, &rows, &HashSet::from([NodeId::Project(one)]), &photo("b", one)));
    }
}
