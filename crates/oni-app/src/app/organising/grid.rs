//! The grid as shown: the bands, the order and the picks the gallery lays.

use super::*;

impl Ctl {
    /// The bands the surface shows, the selection kept to them, then the tiles, the tree's other
    /// rows, the status bar, the sidebar and the empty state.
    pub(in crate::app) fn lay_grid(self: &Rc<Self>) {
        let (remounted, unknown) = {
            let mut s = self.state.borrow_mut();
            let bands = s.bands();
            let order: Vec<String> = bands.iter().flat_map(|b| b.entries.iter().map(|e| e.path.clone())).collect();
            for e in bands.iter().flat_map(|b| &b.entries) {
                s.fingerprints.entry(e.path.clone()).or_insert_with(|| e.fingerprint.clone());
            }
            // The states last published, never the disk: a photo not looked at yet is looked at behind.
            s.org.missing = order.iter().filter(|p| !s.presence(p).is_present()).cloned().collect();
            let unknown: Vec<String> = order.iter().filter(|p| !s.org.presence.contains_key(*p)).cloned().collect();
            for path in &unknown {
                s.org.presence.insert(path.clone(), Default::default());
            }
            let shown: HashSet<String> = order.iter().cloned().collect();
            s.shown_picks_mut().selection.retain(|p| shown.contains(p));
            s.order = order;
            s.org.bands = bands;
            // Each surface is its own `GalleryEngine`, which lands afresh when switched to.
            let surface = if s.org.projects_surface { 1 } else { u8::from(s.trash_shown()) * 2 };
            (s.org.laid_surface.replace(surface) != Some(surface), unknown)
        };
        self.probe(unknown);
        self.relayout();
        if remounted {
            self.land_gallery();
        }
        self.publish_organising();
    }

    pub(in crate::app) fn publish_organising(&self) {
        self.publish_projects();
        self.publish_trash_rows();
        self.publish_status();
        self.publish_info();
        self.publish_empty();
        self.publish_banner();
    }

    pub(super) fn publish_empty(&self) {
        let Some(w) = self.window() else { return };
        let s = self.state.borrow();
        let (title, detail, action) = if !s.order.is_empty() {
            (String::new(), String::new(), String::new())
        } else if s.org.projects_surface {
            (tr("No Photos in a Project").into(), tr("Right-click photos in the library and choose New Project from them.").into(), String::new())
        } else if s.org.trash_open {
            (tr("ONI Trash Is Empty").into(), tr("A photo deleted in ONI waits here, its file untouched, until it is restored.").into(), String::new())
        } else if let Some(m) = s.filter {
            (tr!("No {medal} Photos", medal = library::medal(m)), tr("Cycle the filter (⌘L) to see the others.").into(), String::new())
        } else if s.opened.is_none() {
            (
                tr("No Folder Open").into(),
                tr("Click a folder in the column to open it: every photo under it shows here. Nothing is copied or moved.").into(),
                tr("Open Folder…").into(),
            )
        } else if !s.opened_presence().is_present() {
            // Out of reach, a folder is not empty: the banner over the grid says what it is.
            (String::new(), String::new(), String::new())
        } else {
            (tr("No Photos").into(), tr("No photo under this folder.").into(), tr("Open Folder…").into())
        };
        let lib = w.global::<Library>();
        lib.set_empty_title(title.into());
        lib.set_empty_detail(detail.into());
        lib.set_empty_action(action.into());
        lib.set_projects_empty(if s.org.projects.is_empty() { tr("No Projects") } else { "" }.into());
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
