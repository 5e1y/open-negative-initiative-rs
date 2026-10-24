//! Projects: the Archive and Projects switch, their tree and what a project holds.

use super::*;

impl Ctl {
    pub(in crate::app) fn load_projects(&self) {
        let mut s = self.state.borrow_mut();
        s.org.projects = project::load(&s.support).projects;
        s.org.projects_read = s.org.projects.clone();
    }

    /// `saveProjects`: every member's path written down, then the one file, merged with what
    /// another app wrote since it was read.
    pub(in crate::app) fn save_projects(&self) {
        let failure = {
            let mut s = self.state.borrow_mut();
            let located = project::located(&s.org.projects, &s.rows);
            s.org.projects = located.clone();
            match oni_library::merge::save_projects(&s.support, &s.org.projects_read, &located) {
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

    pub(super) fn new_project(self: &Rc<Self>, paths: &HashSet<String>) {
        let made = {
            let mut s = self.state.borrow_mut();
            let rows = s.rows.clone();
            project::new_from(&mut s.org.projects, &rows, paths, Date::now())
        };
        if made.is_some() {
            self.save_projects();
        }
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
            let nodes = project_tree::flatten(&project_tree::roots(&s.org.projects, &s.rows), &s.org.project_expanded, 0);
            s.org.project_nodes = nodes.iter().map(|(n, _)| n.id()).collect();
            let mut paths = Vec::with_capacity(nodes.len());
            for (node, _) in &nodes {
                let Node::Photo { entry, .. } = node else {
                    paths.push(None);
                    continue;
                };
                // A thumbnail is asked by fingerprint, which the archive's grid may never have read.
                s.fingerprints.entry(entry.path.clone()).or_insert_with(|| entry.fingerprint.clone());
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
                        Node::Photo { entry, .. } => TreeRow {
                            depth: *depth as i32,
                            // `ProjectRow.title`: the photo's whole file name.
                            title: oni_library::paths::name(&entry.path).into(),
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
        let lib = w.global::<Library>();
        lib.set_project_rows(ModelRc::new(VecModel::from(rows)));
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
