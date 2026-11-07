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

    /// The Projects tab's New Project, in its column and on its empty grid: a project with no photo
    /// yet, ringed, its name open to type; one step of the Undo key.
    pub(super) fn new_empty_project(self: &Rc<Self>) {
        let mut made = None;
        self.regroup(tr("New Project"), |projects, _| {
            made = Some(project::new_empty(projects, Date::now()));
            true
        });
        let Some(id) = made else { return };
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
    pub(in crate::app) fn regrouped(self: &Rc<Self>, from: &project::Rosters, to: &project::Rosters, undone: bool) {
        {
            let mut guard = self.state.borrow_mut();
            let s = &mut *guard;
            let found = s.org.projects.clone();
            project::regroup(&mut s.org.projects, from, to);
            if let Some(store) = s.store.editor() {
                store.walked(undone, &found, &s.org.projects);
            }
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

    /// `deleteProjectRequested`: real curation work asks before it goes, counting what it shows.
    pub(super) fn ask_delete_project(self: &Rc<Self>, id: Id) {
        let prompt = {
            let s = self.state.borrow();
            let rows = s.reach();
            let shadowed = project::shadowed(&s.org.projects, &project::Reach::new(&rows, &s.org.found));
            s.org.projects.iter().zip(shadowed).find(|(p, _)| p.id == id).map(|(p, n)| project::delete_prompt(p, n))
        };
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

    /// `TreeEngine` on the projects: the rows laid as the archive's, each roll heading its photos,
    /// a photo as a name or a thumbnail.
    pub(in crate::app) fn publish_projects(&self) {
        let Some(w) = self.window() else { return };
        let side = crate::stocks::row_side(&w);
        let (mut rows, paths): (Vec<TreeRow>, Vec<Option<String>>) = {
            let mut s = self.state.borrow_mut();
            let mut roots = {
                let reached = s.reach();
                project_tree::roots(&s.org.projects, &project::Reach::new(&reached, &s.org.found))
            };
            // The rating filter keeps its one medal in the tree as in the grid, a project with none still shown.
            for root in &mut roots {
                if let Node::Project(g) = root {
                    g.entries.retain(|e| s.filter.is_none_or(|m| e.label == m));
                }
            }
            let nodes = project_tree::flatten(&roots, &s.org.project_expanded, 0);
            s.org.project_nodes = nodes.iter().map(|(n, _)| n.id()).collect();
            let markers = s.roll_markers();
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
                    let depth = *depth as i32;
                    match node {
                        Node::Project(g) => TreeRow {
                            depth,
                            title: g.project.name.clone().into(),
                            caption: g.entries.len().to_string().into(),
                            kind: 1,
                            expandable: !g.entries.is_empty(),
                            expanded: s.org.project_expanded.contains(&id),
                            selected: ringed,
                            renaming: s.org.project_renaming == Some(g.id),
                            ..TreeRow::default()
                        },
                        Node::Roll { folder, entries, .. } => {
                            let (title, caption) = (roll_title(&markers, folder).into(), entries.len().to_string().into());
                            let stock = crate::stocks::row_icon(library::roll_stock(markers.get(folder), entries.first()), side);
                            let has_stock = stock.is_some();
                            TreeRow { depth, title, caption, kind: 3, selected: ringed, has_stock, stock: stock.unwrap_or_default(), ..TreeRow::default() }
                        }
                        Node::Photo { entry, .. } => TreeRow {
                            depth,
                            title: oni_library::paths::name(&entry.path).into(),
                            caption: library::medal(entry.label).into(),
                            kind: 2,
                            selected: ringed || s.picks.selection.contains(&entry.path),
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

/// A roll's name as the archive heads it: its marker's, else its folder's.
fn roll_title(markers: &HashMap<String, Film>, folder: &str) -> String {
    let named = markers.get(folder).map(|f| f.name.trim()).filter(|n| !n.is_empty());
    named.map_or_else(|| oni_library::paths::name(folder), str::to_owned)
}

impl State {
    /// The Projects tab's bands: each project's photos by roll, a band each, headed by the project
    /// and the roll; the rating filter keeps one medal as in the archive, the rolls in the tree's order.
    pub(super) fn project_bands(&self) -> Vec<Band> {
        let markers = self.roll_markers();
        let mut bands = Vec::new();
        for g in self.project_groups() {
            for (folder, entries) in project_tree::by_roll(&g.entries) {
                let entries: Vec<Entry> = entries.into_iter().filter(|e| self.filter.is_none_or(|m| e.label == m)).collect();
                if entries.is_empty() {
                    continue;
                }
                let (title, caption) = (roll_title(&markers, &folder), g.project.name.clone());
                bands.push(Band { key: g.id.to_string(), title, caption, entries, naming: None, explicit: String::new() });
            }
        }
        bands
    }

    /// The photos the projects' tree rows stand for, in the order given, each once: a photo itself,
    /// a roll its project's photos in that folder the filter shows, a project none.
    pub(in crate::app) fn row_photos<'a>(&self, rows: impl IntoIterator<Item = &'a NodeId>) -> Vec<String> {
        let groups = self.project_groups();
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for row in rows {
            let paths: Vec<String> = match row {
                NodeId::Photo { path, .. } => vec![path.clone()],
                NodeId::Roll { folder, project } => {
                    let entries = groups.iter().filter(|g| g.id == *project).flat_map(|g| &g.entries);
                    let shown = entries.filter(|e| self.filter.is_none_or(|m| e.label == m));
                    shown.filter(|e| oni_library::paths::parent(&e.path) == *folder).map(|e| e.path.clone()).collect()
                }
                NodeId::Project(_) => Vec::new(),
            };
            out.extend(paths.into_iter().filter(|p| seen.insert(p.clone())));
        }
        out
    }
}
