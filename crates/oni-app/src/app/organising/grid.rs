//! The grid as shown: the bands, the order and the picks the gallery lays.

use super::*;

impl Ctl {
    /// The bands the surface shows, the selection kept to them, then the tiles, the tree's other
    /// rows, the status bar, the sidebar and the empty state.
    pub(in crate::app) fn lay_grid(self: &Rc<Self>) {
        {
            let mut s = self.state.borrow_mut();
            let bands = s.bands();
            let order: Vec<String> = bands.iter().flat_map(|b| b.entries.iter().map(|e| e.path.clone())).collect();
            for e in bands.iter().flat_map(|b| &b.entries) {
                s.fingerprints.entry(e.path.clone()).or_insert_with(|| e.fingerprint.clone());
            }
            s.org.missing = order.iter().filter(|p| !Path::new(p).exists()).cloned().collect();
            let shown: HashSet<String> = order.iter().cloned().collect();
            s.shown_picks_mut().selection.retain(|p| shown.contains(p));
            s.order = order;
            s.org.bands = bands;
        }
        self.relayout();
        self.publish_organising();
    }

    pub(in crate::app) fn publish_organising(&self) {
        self.publish_projects();
        self.publish_trash_rows();
        self.publish_status();
        self.publish_info();
        self.publish_empty();
    }

    pub(super) fn publish_empty(&self) {
        let Some(w) = self.window() else { return };
        let s = self.state.borrow();
        let (title, detail, action) = if !s.order.is_empty() {
            (String::new(), String::new(), String::new())
        } else if s.org.projects_surface {
            ("No Photos in a Project".into(), "Right-click photos in the library and choose New Project from them.".into(), String::new())
        } else if s.org.trash_open {
            ("ONI Trash Is Empty".into(), "A photo deleted in ONI waits here, its file untouched, until it is restored.".into(), String::new())
        } else if let Some(m) = s.filter {
            (format!("No {} Photos", library::medal(m)), "Cycle the filter (⌘L) to see the others.".into(), String::new())
        } else if s.opened.is_none() {
            (
                "No Folder Open".into(),
                "Double-click a folder in the column to open it: every photo under it shows here. Nothing is copied or moved.".into(),
                "Open Folder…".into(),
            )
        } else {
            ("No Photos".into(), "No photo under this folder.".into(), "Open Folder…".into())
        };
        let lib = w.global::<Library>();
        lib.set_empty_title(title.into());
        lib.set_empty_detail(detail.into());
        lib.set_empty_action(action.into());
        lib.set_projects_empty(if s.org.projects.is_empty() { "No Projects" } else { "" }.into());
    }

    /// The tile flags the bands carry: medal, missing original, a triplet's lead.
    pub(in crate::app) fn tile_flags(s: &State) -> HashMap<String, (i32, bool, bool)> {
        s.org
            .bands
            .iter()
            .flat_map(|b| &b.entries)
            .map(|e| (e.path.clone(), (library::medal_index(e.label), s.org.missing.contains(&e.path), e.leads_passes)))
            .collect()
    }
}
