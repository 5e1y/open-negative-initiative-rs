//! Projects: the Archive and Projects switch, their tree and what a project holds.

use super::*;

impl Ctl {
    pub(in crate::app) fn load_projects(&self) {
        let mut s = self.state.borrow_mut();
        s.org.projects = project::load(&s.support).projects;
        s.org.projects_read = s.org.projects.clone();
    }

    /// `saveProjects`: the one file, merged with what another app wrote since it was read.
    pub(in crate::app) fn save_projects(&self) {
        let failure = {
            let mut s = self.state.borrow_mut();
            let ours = s.org.projects.clone();
            match oni_library::merge::save_projects(&s.support, &s.org.projects_read, &ours) {
                Ok(written) => {
                    s.org.projects_read = written.clone();
                    s.org.projects = written;
                    None
                }
                Err(e) => Some(e),
            }
        };
        if let Some(e) = failure {
            self.told(tr("Something Went Wrong"), &tr!("The projects could not be saved: {error}", error = e));
        }
    }

    /// New Project from photos: one step of the Undo key, each photo leaving the project it sat in;
    /// none picked makes none.
    pub(super) fn new_project(self: &Rc<Self>, paths: &HashSet<String>) -> Option<Id> {
        let mut made = None;
        self.regroup(tr("New Project"), |projects, reach| {
            made = project::new_from(projects, reach, paths, Date::now());
            made.is_some()
        });
        made
    }

    /// The empty Projects tab's key: a project with no photo yet, ringed, its name open to type.
    pub(super) fn new_empty_project(self: &Rc<Self>) {
        let id = project::new_empty(&mut self.state.borrow_mut().org.projects, Date::now());
        self.save_projects();
        {
            let mut s = self.state.borrow_mut();
            s.org.project_ring = Some(NodeId::Project(id));
            s.org.project_renaming = Some(id);
        }
        self.lay_grid();
    }

    /// Every gesture on the projects' members, a drag and a drop included: one step the Undo key
    /// takes back, saved and laid; whether it moved any.
    pub(in crate::app) fn regroup(self: &Rc<Self>, label: &str, gesture: impl FnOnce(&mut Vec<Project>, &project::Reach) -> bool) -> bool {
        let moved = {
            let mut guard = self.state.borrow_mut();
            let s = &mut *guard;
            let rows = s.reach().into_owned();
            let before = project::rosters(&s.org.projects);
            let moved = gesture(&mut s.org.projects, &project::Reach::new(&rows, &s.org.found));
            let members = oni_library::history::Members { before, after: project::rosters(&s.org.projects) };
            if let Some(store) = s.store.editor().filter(|_| moved) {
                store.record_members(members, label);
            }
            moved
        };
        if moved {
            self.save_projects();
            self.lay_grid();
            self.publish_bar();
        }
        moved
    }

    /// An undo or a redo of a gesture on the projects: what its step changed, turned on the members
    /// as they stand, saved; where they stand read again, as one it brings back may lie beyond the folder.
    pub(in crate::app) fn regrouped(self: &Rc<Self>, from: &project::Rosters, to: &project::Rosters) {
        {
            let mut s = self.state.borrow_mut();
            project::regroup(&mut s.org.projects, from, to);
            let ringed = |id: &Id| s.org.projects.iter().any(|p| p.id == *id);
            if matches!(&s.org.project_ring, Some(NodeId::Project(r)) if !ringed(r)) {
                s.org.project_ring = None;
            }
        }
        self.save_projects();
        self.read_standing();
        self.lay_grid();
        self.publish_bar();
    }

    /// `deleteProjectRequested`: real curation work asks before it goes.
    pub(super) fn ask_delete_project(self: &Rc<Self>, id: Id) {
        let prompt = self.state.borrow().org.projects.iter().find(|p| p.id == id).map(project::delete_prompt);
        if let Some((title, message, key)) = prompt {
            self.ask(&title, &message, key, true, Pending::DeleteProject(id));
        }
    }

    pub(in crate::app) fn delete_project(self: &Rc<Self>, id: Id) {
        {
            let mut s = self.state.borrow_mut();
            project::delete(&mut s.org.projects, id);
            if matches!(s.org.project_ring, Some(NodeId::Project(r)) if r == id) {
                s.org.project_ring = None;
            }
        }
        self.save_projects();
        self.lay_grid();
    }

    pub(in crate::app) fn surface_picked(self: &Rc<Self>, i: i32) {
        {
            let mut s = self.state.borrow_mut();
            s.org.projects_surface = i == 1;
            s.org.reorder = None;
            s.org.renaming = None;
            s.org.info_loaded = None;
            // Each tab's tree is made anew, its own picks and fields with it.
            s.org.tree_pick = TreePick::default();
            s.org.project_pick = TreePick::default();
            s.org.project_renaming = None;
            s.tree_renaming = None;
            s.org.tree_draft = None;
        }
        if let Some(w) = self.window() {
            w.global::<Library>().set_surface(i);
        }
        self.lay_grid();
        self.publish_bar();
    }

    /// `TreeEngine` on the projects: the rows laid as the archive's, a photo as a name or a thumbnail.
    pub(in crate::app) fn publish_projects(&self) {
        let Some(w) = self.window() else { return };
        let (mut rows, paths): (Vec<TreeRow>, Vec<Option<String>>) = {
            let mut s = self.state.borrow_mut();
            let roots = {
                let reached = s.reach();
                project_tree::roots(&s.org.projects, &project::Reach::new(&reached, &s.org.found))
            };
            let nodes = project_tree::flatten(&roots, &s.org.project_expanded, 0);
            s.org.project_nodes = nodes.iter().map(|(n, _)| n.id()).collect();
            let titles = photo_titles(&roots);
            let mut paths = Vec::with_capacity(nodes.len());
            for (node, _) in &nodes {
                let Node::Photo { entry, .. } = node else {
                    paths.push(None);
                    continue;
                };
                // A thumbnail is asked by fingerprint, which the archive's grid may never have read.
                let key = s.tile_key(entry);
                s.fingerprints.entry(entry.path.clone()).or_insert(key);
                paths.push(Some(entry.path.clone()));
            }
            let rows = nodes
                .iter()
                .map(|(node, depth)| {
                    let id = node.id();
                    let ringed = s.org.project_ring.as_ref() == Some(&id) || s.org.project_pick.selection.contains(&id);
                    match node {
                        Node::Project(g) => TreeRow {
                            depth: *depth as i32,
                            title: g.project.name.clone().into(),
                            caption: g.entries.len().to_string().into(),
                            kind: 1,
                            expandable: !g.entries.is_empty(),
                            expanded: s.org.project_expanded.contains(&id),
                            selected: ringed,
                            renaming: s.org.project_renaming == Some(g.id),
                            ..TreeRow::default()
                        },
                        Node::Photo { entry, project } => TreeRow {
                            depth: *depth as i32,
                            title: titles.get(&(*project, entry.path.clone())).cloned().unwrap_or_default().into(),
                            caption: library::medal(entry.label).into(),
                            kind: 2,
                            expandable: false,
                            expanded: false,
                            selected: ringed || s.picks.selection.contains(&entry.path),
                            renaming: false,
                            medal: library::medal_index(entry.label),
                            missing: s.org.missing.contains(&entry.path),
                            rgb: entry.leads_passes,
                            ..TreeRow::default()
                        },
                    }
                })
                .collect();
            (rows, paths)
        };
        self.lay_column(crate::app::tree_column::Which::Projects, &mut rows, paths);
        self.deliver_column(crate::app::tree_column::Which::Projects, rows);
        let lib = w.global::<Library>();
        lib.set_projects_any_open(!self.state.borrow().org.project_expanded.is_empty());
        self.pin_column(crate::app::tree_column::Which::Projects);
    }

    /// `foldOrOpenEveryRow` on the projects: any row open folds them all, else every project opens.
    pub(in crate::app) fn project_fold_all(&self) {
        {
            let mut s = self.state.borrow_mut();
            if s.org.project_expanded.is_empty() {
                let all: HashSet<NodeId> = s.org.projects.iter().map(|p| NodeId::Project(p.id.clone())).collect();
                s.org.project_expanded = all;
            } else {
                s.org.project_expanded.clear();
            }
        }
        self.publish_projects();
    }
}

/// `ProjectRow.title`: the photo's whole file name, its roll's folder beside it where another photo
/// of the project bears the same, as two rolls' copies of one file do.
fn photo_titles(roots: &[Node]) -> HashMap<(Id, String), String> {
    let mut titles = HashMap::new();
    for root in roots {
        let Node::Project(group) = root else { continue };
        let named: Vec<(&Entry, String)> = group.entries.iter().map(|e| (e, oni_library::paths::name(&e.path))).collect();
        let mut seen: HashMap<&str, usize> = HashMap::new();
        for (_, name) in &named {
            *seen.entry(name.as_str()).or_default() += 1;
        }
        for (e, name) in &named {
            let title = if seen[name.as_str()] > 1 { format!("{name} \u{b7} {}", oni_library::paths::name(&oni_library::paths::parent(&e.path))) } else { name.clone() };
            titles.insert((group.id, e.path.clone()), title);
        }
    }
    titles
}
