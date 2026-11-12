//! A drag in the projects' tree (`TreeEngine.swift:234-244,458-552`, `EditorScene+Rolls.swift:202-271`):
//! the pick or the row pressed, onto a project to join it or between two rows, to `projects.json` alone.

use super::organising::DRAG_DISTANCE;
use super::tree_column::Which;
use super::*;
use crate::TreeDrag;
use crate::sidebar::{self, DropTarget};
use oni_library::project;
use oni_library::project_tree::NodeId;

/// A drag under way: the rows it carries, the pointer in the tree's coordinates, the row whose press
/// it is, the place last pointed at with where a release there lands, none where it changes nothing.
#[derive(Clone, Debug)]
pub(super) struct Drag {
    items: HashSet<NodeId>,
    at: (f32, f32),
    row: usize,
    target: Option<(DropTarget, Option<Landing>)>,
}

/// The photos a drag carries, its projects' rows left out.
fn photos(items: &HashSet<NodeId>) -> HashSet<String> {
    items.iter().filter_map(|n| if let NodeId::Photo { path, .. } = n { Some(path.clone()) } else { None }).collect()
}

/// What a release does: photos join a project, or a roll's heading of their own roll, or land beside
/// a photo of their roll, past it when it ends the run above the line; projects land before a project.
#[derive(Clone, Debug, PartialEq)]
enum Landing {
    Join(oni_settings::Id),
    Beside(String, bool),
    Before(oni_settings::Id),
}

/// Where a release at `target` lands `items` among the tree's `nodes`, none where it changes no
/// project, so no mark promises it.
fn landing(nodes: &[NodeId], projects: &[project::Project], reach: &project::Reach, items: &HashSet<NodeId>, target: DropTarget) -> Option<Landing> {
    let aimed = aimed(nodes, items, target)?;
    land(&mut projects.to_vec(), reach, items, &aimed).then_some(aimed)
}

/// What a release at `target` aims at: a project, a roll's heading of the photos' roll, a photo of
/// their roll; a line above a heading or a project ends the roll above it.
fn aimed(nodes: &[NodeId], items: &HashSet<NodeId>, target: DropTarget) -> Option<Landing> {
    let paths = photos(items);
    let roll_of = |folder: &str| !paths.is_empty() && paths.iter().all(|p| oni_library::paths::parent(p) == folder);
    let photo = |j: usize| match nodes.get(j) {
        Some(NodeId::Photo { path, .. }) => Some(path.clone()),
        _ => None,
    };
    match (target, nodes.get(target_row(target))?) {
        (DropTarget::Join(_), node) if items.contains(node) => None,
        (DropTarget::Join(_), NodeId::Project(id)) => (!paths.is_empty()).then_some(Landing::Join(*id)),
        (DropTarget::Join(_), NodeId::Roll { folder, project }) => roll_of(folder).then_some(Landing::Join(*project)),
        (DropTarget::Join(_), NodeId::Photo { .. }) => None,
        (DropTarget::Before(_), NodeId::Project(id)) if items.iter().any(|n| matches!(n, NodeId::Project(_))) => Some(Landing::Before(*id)),
        (DropTarget::Before(_), _) if paths.is_empty() => None,
        (DropTarget::Before(i), node) => {
            let under = || matches!(node, NodeId::Roll { .. }).then(|| photo(i + 1)).flatten().map(|p| (p, false));
            let (path, past) = photo(i).map(|p| (p, false)).or_else(|| i.checked_sub(1).and_then(photo).map(|p| (p, true))).or_else(under)?;
            project::lands_beside(&paths, &path).then_some(Landing::Beside(path, past))
        }
    }
}

fn target_row(target: DropTarget) -> usize {
    match target {
        DropTarget::Join(i) | DropTarget::Before(i) => i,
    }
}

/// `addToProjectDrop` and `reorderProjectTreeDrop` on the projects: what the landing changed.
fn land(projects: &mut [project::Project], reach: &project::Reach, items: &HashSet<NodeId>, landing: &Landing) -> bool {
    match landing {
        Landing::Join(id) => project::add_to(projects, reach, &photos(items), *id).added > 0,
        Landing::Beside(path, past) => project::reorder_drop(projects, reach, &photos(items), path, *past),
        Landing::Before(id) => {
            let ids: HashSet<oni_settings::Id> = items.iter().filter_map(|n| if let NodeId::Project(p) = n { Some(*p) } else { None }).collect();
            project::reorder_root(projects, &ids, *id)
        }
    }
}

impl Ctl {
    /// A press on row `i` moved from `from` to `at`: past the drag distance it carries the pick when
    /// the row is in it, else the row (`dragged()`); each move resolves where it would land.
    pub(super) fn project_row_dragged(&self, i: usize, from: (f32, f32), at: (f32, f32)) {
        {
            let mut guard = self.state.borrow_mut();
            let s = &mut *guard;
            if s.org.project_drag.is_none() {
                if (at.0 - from.0).hypot(at.1 - from.1) < DRAG_DISTANCE {
                    return;
                }
                let Some(id) = s.org.project_nodes.get(i).cloned() else { return };
                let pick = &s.org.project_pick.selection;
                let rows = if pick.contains(&id) { pick.clone() } else { HashSet::from([id]) };
                // A roll's heading carries its photos.
                let items = rows
                    .iter()
                    .flat_map(|row| match row {
                        NodeId::Roll { project, .. } => s.row_photos([row]).into_iter().map(|path| NodeId::Photo { path, project: *project }).collect(),
                        _ => vec![row.clone()],
                    })
                    .collect();
                s.org.project_drag = Some(Drag { items, at, row: i, target: None });
            }
            if let Some(d) = s.org.project_drag.as_mut() {
                d.at = at;
            }
        }
        self.aim_project_drag(false);
    }

    /// Where a release under the held drag's pointer lands on the tree as laid now, its mark drawn: the
    /// place pointed at keeps its landing until the tree is `laid` anew, an undo or a read changing its rows.
    pub(super) fn aim_project_drag(&self, laid: bool) {
        let Some(w) = self.window() else { return };
        let strip = w.global::<crate::DS>().get_space_md() / 2.0;
        {
            let mut guard = self.state.borrow_mut();
            let s = &mut *guard;
            let Some(d) = s.org.project_drag.as_ref() else { return };
            let l = &s.project_tree;
            let pointed = sidebar::drop_target(d.at.0, d.at.1, &l.frames, &l.photos, &l.depths, true, strip);
            let target = match &d.target {
                Some((t, landed)) if !laid && Some(*t) == pointed => Some((*t, landed.clone())),
                _ => pointed.map(|t| {
                    let rows = s.reach();
                    (t, landing(&s.org.project_nodes, &s.org.projects, &project::Reach::new(&rows, &s.org.found), &d.items, t))
                }),
            };
            if let Some(d) = s.org.project_drag.as_mut() {
                d.target = target;
            }
        }
        self.publish_project_drag();
    }

    /// The tree laid again under a held drag: its row follows its place, and a row the re-lay removed
    /// took its press along, so the drag ends there, dropping nothing.
    pub(super) fn follow_project_drag(&self, relaid: super::tree_rows::Relaid) {
        {
            let mut s = self.state.borrow_mut();
            let Some(d) = s.org.project_drag.as_mut() else { return };
            if let Some(row) = relaid.kept(d.row) {
                d.row = row;
                return;
            }
            s.org.project_drag = None;
        }
        self.publish_project_drag();
    }

    /// The press let go: `performDrop` on where the drag would land; a cancel drops nothing. What
    /// the drop changes in the projects' members is one step of the Undo key, as Add to Project is.
    pub(super) fn project_drag_ended(self: &Rc<Self>, dropped: bool) {
        let Some(drag) = self.state.borrow_mut().org.project_drag.take() else { return };
        self.publish_project_drag();
        let Some(landed) = drag.target.and_then(|(_, landed)| landed).filter(|_| dropped) else { return };
        let label = if matches!(landed, Landing::Join(_)) { tr("Add to Project") } else { tr("Move") };
        self.regroup(label, |projects, reach| land(projects, reach, &drag.items, &landed));
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
                Some((DropTarget::Join(i), Some(_))) => (i, true),
                Some((DropTarget::Before(i), Some(_))) => (i, false),
                _ => return mark,
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
    use oni_library::project::{Member, Project, Reach};
    use oni_settings::Date;

    fn entry(path: &str) -> oni_library::Entry {
        oni_library::Entry::new(path, Date::since_reference_date(8e8), &format!("f-{path}"))
    }

    fn of(path: &str) -> Member {
        Member::new(Some(path), Some(&format!("f-{path}")))
    }

    /// The tree's rows as `flatten` lays every project open: its row, each roll's heading, its photos.
    fn nodes(projects: &[Project], reach: &Reach) -> Vec<NodeId> {
        let tops = oni_library::project_tree::roots(projects, reach);
        let open: HashSet<NodeId> = tops.iter().map(|n| n.id()).collect();
        oni_library::project_tree::flatten(&tops, &open, 0).into_iter().map(|(n, _)| n.id()).collect()
    }

    /// A release at `target` of the row `node` stands at, as the tree lays the projects now.
    fn dropped(projects: &mut [Project], reach: &Reach, items: &HashSet<NodeId>, target: fn(usize) -> DropTarget, node: &NodeId) -> bool {
        let rows = nodes(projects, reach);
        let at = rows.iter().position(|n| n == node).expect("the row laid");
        landing(&rows, projects, reach, items, target(at)).is_some_and(|l| land(projects, reach, items, &l))
    }

    #[test]
    fn a_photo_dropped_on_a_project_joins_it_and_before_a_photo_lands_there() {
        let rows: Vec<_> = ["/r/a", "/r/b", "/r/c"].iter().map(|p| entry(p)).collect();
        let reach = Reach::rows(&rows);
        let at = |m: f64| Date::since_reference_date(8e8 + m);
        let mut projects = vec![Project::new("One", at(2.0), vec![of("/r/a"), of("/r/b")]), Project::new("Two", at(1.0), vec![of("/r/c")])];
        let (one, two) = (projects[0].id, projects[1].id);
        let photo = |p: &str, project| NodeId::Photo { path: p.into(), project };
        let c = HashSet::from([photo("/r/c", two)]);
        // Twin: a photo dropped onto a photo joins nothing; only a project takes a member.
        assert!(!dropped(&mut projects, &reach, &c, DropTarget::Join, &photo("/r/a", one)));
        assert!(dropped(&mut projects, &reach, &c, DropTarget::Join, &NodeId::Project(one)));
        assert!(projects.len() == 2 && projects[1].members.is_empty(), "Two, emptied, stays with its name");
        assert_eq!(projects[0].paths(), ["/r/a", "/r/b", "/r/c"]);
        let c = HashSet::from([photo("/r/c", one)]);
        assert!(dropped(&mut projects, &reach, &c, DropTarget::Before, &photo("/r/a", one)));
        assert_eq!(projects[0].paths(), ["/r/c", "/r/a", "/r/b"], "before a photo, in its place");
        // Twin: a project dropped before a photo moves nothing.
        assert!(!dropped(&mut projects, &reach, &HashSet::from([NodeId::Project(two)]), DropTarget::Before, &photo("/r/b", one)));
        // A roll's heading takes a photo of its roll into its project, as the project's row does.
        projects[1].members.push(of("/r/b2"));
        let rows: Vec<_> = ["/r/a", "/r/b", "/r/c", "/r/b2"].iter().map(|p| entry(p)).collect();
        let reach = Reach::rows(&rows);
        let heading = NodeId::Roll { folder: "/r".into(), project: two };
        let b = HashSet::from([photo("/r/b", one)]);
        assert!(dropped(&mut projects, &reach, &b, DropTarget::Join, &heading));
        assert_eq!(projects[1].paths(), ["/r/b2", "/r/b"]);
    }

    /// Roll 10 shows A C D above Roll 8's A B: the line above Roll 8's heading ends Roll 10, so A
    /// dragged there reads C D A; a photo of Roll 8 lands nowhere there, nor a heading of another roll.
    #[test]
    fn the_line_above_a_roll_s_heading_ends_the_roll_above_it() {
        let paths = ["/s/Roll 10/A", "/s/Roll 10/C", "/s/Roll 8/A", "/s/Roll 8/B", "/s/Roll 10/D", "/s/Roll 8/Z"];
        let rows: Vec<_> = paths.iter().map(|p| entry(p)).collect();
        let reach = Reach::rows(&rows);
        let client = Project::new("Client", Date::since_reference_date(8e8), paths[..5].iter().map(|p| of(p)).collect());
        let other = Project::new("Other", Date::since_reference_date(7e8), vec![of(paths[5])]);
        let (id, other_id) = (client.id, other.id);
        let mut projects = vec![client, other];
        let row = |projects: &[Project], node: NodeId| nodes(projects, &reach).iter().position(|n| *n == node).unwrap();
        let heading = |folder: &str, project| NodeId::Roll { folder: folder.into(), project };
        let photo = |p: &str, project| NodeId::Photo { path: p.into(), project };
        let a = HashSet::from([photo(paths[0], id)]);
        let above = DropTarget::Before(row(&projects, heading("/s/Roll 8", id)));
        assert_eq!(landing(&nodes(&projects, &reach), &projects, &reach, &a, above), Some(Landing::Beside(paths[4].into(), true)));
        assert!(dropped(&mut projects, &reach, &a, DropTarget::Before, &heading("/s/Roll 8", id)));
        assert_eq!(projects[0].paths(), ["/s/Roll 10/C", "/s/Roll 10/D", "/s/Roll 10/A", "/s/Roll 8/A", "/s/Roll 8/B"], "Roll 10 reads C D A");
        // Twin: the line beside Roll 8's first photo takes no photo of Roll 10.
        let first = DropTarget::Before(row(&projects, photo(paths[2], id)));
        assert_eq!(landing(&nodes(&projects, &reach), &projects, &reach, &a, first), None);
        // The line above Other's row ends Client's last roll for a photo of it; it never joins Other.
        let b = HashSet::from([photo(paths[2], id)]);
        assert!(dropped(&mut projects, &reach, &b, DropTarget::Before, &NodeId::Project(other_id)));
        assert_eq!(projects[0].paths()[3..], ["/s/Roll 8/B", "/s/Roll 8/A"]);
        assert_eq!(projects[1].paths(), [paths[5]], "Other unchanged");
        // Roll 10's heading takes no photo of Roll 8; Other's Roll 8 heading does.
        let laid = nodes(&projects, &reach);
        let z = HashSet::from([photo(paths[5], other_id)]);
        assert_eq!(landing(&laid, &projects, &reach, &z, DropTarget::Join(row(&projects, heading("/s/Roll 10", id)))), None);
        assert_eq!(landing(&laid, &projects, &reach, &b, DropTarget::Join(row(&projects, heading("/s/Roll 8", other_id)))), Some(Landing::Join(other_id)));
    }

    /// Client holds Roll 10's A and C above Other: A onto Client's row, onto its roll's heading or on
    /// the line above C lands nowhere, nor Client on the line above Other, as nothing would change.
    #[test]
    fn a_drop_lands_only_where_it_changes_a_project() {
        let paths = ["/s/Roll 10/A", "/s/Roll 10/C", "/s/Roll 9/X"];
        let rows: Vec<_> = paths.iter().map(|p| entry(p)).collect();
        let reach = Reach::rows(&rows);
        let client = Project::new("Client", Date::since_reference_date(8e8), vec![of(paths[0]), of(paths[1])]);
        let other = Project::new("Other", Date::since_reference_date(7e8), vec![of(paths[2])]);
        let (id, other_id) = (client.id, other.id);
        let projects = vec![client, other];
        let laid = nodes(&projects, &reach);
        let row = |node: NodeId| laid.iter().position(|n| *n == node).unwrap();
        let a = HashSet::from([NodeId::Photo { path: paths[0].into(), project: id }]);
        let lands = |items: &HashSet<NodeId>, target| landing(&laid, &projects, &reach, items, target);
        assert_eq!(lands(&a, DropTarget::Join(row(NodeId::Project(id)))), None, "A is Client's already");
        assert_eq!(lands(&a, DropTarget::Join(row(NodeId::Roll { folder: "/s/Roll 10".into(), project: id }))), None);
        assert_eq!(lands(&a, DropTarget::Before(row(NodeId::Photo { path: paths[1].into(), project: id }))), None, "A stands above C");
        let projects_row = |p| HashSet::from([NodeId::Project(p)]);
        assert_eq!(lands(&projects_row(id), DropTarget::Before(row(NodeId::Project(other_id)))), None, "Client stands above Other");
        // Twins: Other's row takes A, the line below C moves it, and Other above Client moves Other.
        assert_eq!(lands(&a, DropTarget::Join(row(NodeId::Project(other_id)))), Some(Landing::Join(other_id)));
        assert_eq!(lands(&a, DropTarget::Before(row(NodeId::Project(other_id)))), Some(Landing::Beside(paths[1].into(), true)));
        assert_eq!(lands(&projects_row(other_id), DropTarget::Before(row(NodeId::Project(id)))), Some(Landing::Before(id)));
    }
}
