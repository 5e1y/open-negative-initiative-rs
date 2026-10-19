//! Projects: the Archive and Projects switch, their tree and what a project holds.

use super::*;

impl Ctl {
    pub(in crate::app) fn load_projects(&self) {
        let mut s = self.state.borrow_mut();
        s.org.projects = project::load(&s.support).projects;
    }

    /// `saveProjects`: every member's path written down, then the one file.
    pub(super) fn save_projects(&self) {
        let failure = {
            let mut s = self.state.borrow_mut();
            let located = project::located(&s.org.projects, &s.rows);
            s.org.projects = located.clone();
            project::save(&s.support, &ProjectsFile { version: 1, projects: located }).err()
        };
        if let Some(e) = failure {
            self.say(&format!("The projects could not be saved: {e}"));
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
            s.org.info_loaded = None;
        }
        if let Some(w) = self.window() {
            w.global::<Library>().set_surface(i);
        }
        self.lay_grid();
        self.publish_bar();
    }

    pub(in crate::app) fn publish_projects(&self) {
        let Some(w) = self.window() else { return };
        let rows: Vec<TreeRow> = {
            let mut s = self.state.borrow_mut();
            let nodes = project_tree::flatten(&project_tree::roots(&s.org.projects, &s.rows), &s.org.project_expanded, 0);
            s.org.project_nodes = nodes.iter().map(|(n, _)| n.id()).collect();
            nodes
                .iter()
                .map(|(node, depth)| {
                    let id = node.id();
                    let ringed = s.org.project_ring.as_ref() == Some(&id);
                    match node {
                        Node::Project(g) => TreeRow {
                            depth: *depth as i32,
                            title: g.project.name.clone().into(),
                            caption: g.entries.len().to_string().into(),
                            kind: 1,
                            expandable: !g.entries.is_empty(),
                            expanded: s.org.project_expanded.contains(&id),
                            selected: ringed,
                            renaming: false,
                            ..TreeRow::default()
                        },
                        Node::Photo { entry, .. } => TreeRow {
                            depth: *depth as i32,
                            title: stem(&entry.path).into(),
                            caption: library::medal(entry.label).into(),
                            kind: 2,
                            expandable: false,
                            expanded: false,
                            selected: ringed || s.picks.selection.contains(&entry.path),
                            renaming: false,
                            ..TreeRow::default()
                        },
                    }
                })
                .collect()
        };
        let lib = w.global::<Library>();
        lib.set_project_rows(ModelRc::new(VecModel::from(rows)));
        lib.set_projects_any_open(!self.state.borrow().org.project_expanded.is_empty());
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

    /// `projectContextMenu`: Delete Project on a project, Edit then the cell menu on a photo.
    pub(in crate::app) fn project_row_menu(self: &Rc<Self>, i: usize) -> bool {
        let id = self.state.borrow().org.project_nodes.get(i).cloned();
        match id {
            Some(NodeId::Project(id)) => {
                let item = Item { line: menu_line("Delete Project", true, false, false), act: Act::DeleteProject(id) };
                self.open_menu(vec![item], None)
            }
            Some(NodeId::Photo { path, .. }) => {
                let can_paste = self.can_paste();
                let items = {
                    let s = self.state.borrow();
                    let scope = organise::cell_scope(&path, &s.picks.selection);
                    self.cell_items(&s, &path, &scope, true, can_paste)
                };
                self.open_menu(items, Some(path))
            }
            None => false,
        }
    }
}
