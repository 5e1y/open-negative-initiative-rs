//! What the state answers: the reads the panel, the benches and the checks make of it.

use super::*;

impl State {
    /// The gallery's picks as places in its order, which a batch walks.
    pub(crate) fn selection(&self) -> std::collections::BTreeSet<usize> {
        self.order.iter().enumerate().filter(|(_, p)| self.picks.selection.contains(*p)).map(|(i, _)| i).collect()
    }

    /// `TreeEngine.marked`: the selection the gallery shares, and the ring (`TreeEngine.swift:247`).
    pub(super) fn tree_marks(&self, path: &str) -> bool {
        self.picks.selection.contains(path) || self.picks.anchor.as_deref() == Some(path)
    }

    /// `designate(_:)`, which every load runs: the frame opened is the selection, cursor and ring.
    pub(super) fn designate(&mut self, path: &str) {
        // Read before the designation narrows it: a frame never measured prepares this selection.
        let among = self.picks.selection.len() > 1 && self.picks.selection.contains(path);
        self.prepare.opened_among = if among { self.picks.selection.clone() } else { HashSet::new() };
        self.picks.selection = HashSet::from([path.to_string()]);
        self.picks.anchor = Some(path.to_string());
        self.picks.cursor = Some(path.to_string());
    }

    /// The settings the canvas was last asked to show, a hovered preset's preview included.
    pub fn shown_settings(&self) -> Option<&oni_settings::PipelineSettings> {
        self.requested_live.as_ref()
    }

    pub fn renders(&self) -> usize {
        self.renders
    }

    /// The store revision the canvas last showed.
    pub fn rendered(&self) -> u64 {
        self.rendered
    }

    pub fn store(&self) -> &dyn Settings {
        self.store.as_ref()
    }

    pub fn open_path(&self) -> Option<&str> {
        self.open.as_deref()
    }

    /// The thumbnails of the opened folder settled so far: held in the folder, drawn, or refused.
    pub fn loaded(&self) -> usize {
        self.painted.0
    }

    /// Whether the opened folder's read has landed whole.
    pub fn read_done(&self) -> bool {
        self.read_done
    }

    pub fn opened(&self) -> Option<&str> {
        self.opened.as_deref()
    }

    pub fn filter(&self) -> Option<Medal> {
        self.filter
    }

    /// The thumbnails in memory dropped, as a long scroll drops them, so a check sees a tile read
    /// back from the folder.
    pub fn forget_tiles(&mut self) {
        self.thumbs.clear();
        self.asked.clear();
    }

    /// The photo's fingerprint as the index holds it, which names its thumbnail's file.
    pub fn fingerprint(&self, path: &str) -> Option<&str> {
        self.fingerprints.get(path).map(String::as_str)
    }

    /// Whether a thumbnail redraw is queued or drawing: an edit's, never the pass's.
    pub fn drawing(&self) -> bool {
        self.drawing > 0
    }

    /// The photos the open folder holds, which the thumbnails count up to.
    pub fn photos(&self) -> usize {
        self.order.len()
    }

    pub fn visible(&self) -> std::ops::Range<usize> {
        self.visible.clone()
    }

    pub fn brush(&self) -> f32 {
        self.brush
    }

    /// The notch renders shown so far, the index `Notches::summary` starts from.
    pub fn notches(&self) -> &Notches {
        &self.notches
    }

    /// The zooms and pans shown, timed as the notches are.
    pub fn views(&self) -> &Notches {
        &self.views
    }

    pub fn placed(&self) -> Option<Placed> {
        self.placed
    }

    pub fn viewport(&self) -> Viewport {
        self.viewport
    }

    pub fn split(&self) -> Option<f32> {
        self.split
    }

    pub fn befores(&self) -> usize {
        self.befores
    }

    pub fn phone(&self) -> Option<&Phone> {
        self.phone.as_ref()
    }

    /// `loadBar`: the thumbnails being created, else the library being read, with their counts.
    pub fn load_bar(&self) -> Option<String> {
        let (done, total) = self.creating;
        if done < total {
            return Some(format!("Creating thumbnails · {done} of {total}"));
        }
        if !self.read_done && self.opened.is_some() {
            let waiting = self.rows.iter().filter(|e| e.fingerprint.starts_with(thumbnails::PENDING)).count();
            let total = self.rows.len();
            return Some(format!("Reading the library · {} of {total}", total - waiting));
        }
        self.making_sheet.map(|m| m.bar())
    }

    /// The open frame's place in its group, one-based, and the group's count.
    pub(super) fn place(&self) -> Option<(u16, u16)> {
        let open = self.open_path()?;
        let group = self.archive.as_ref()?.groups.iter().find(|g| g.entries.iter().any(|e| e.path == open))?;
        let at = group.entries.iter().position(|e| e.path == open)?;
        Some((at as u16 + 1, group.entries.len() as u16))
    }
}
