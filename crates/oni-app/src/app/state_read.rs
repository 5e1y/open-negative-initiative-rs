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

    /// The open frame's full resolution, once built.
    pub fn full_built(&self) -> bool {
        self.full.as_ref().is_some_and(|(p, _)| self.open.as_ref() == Some(p))
    }

    /// The settings the canvas was last asked to show, a hovered preset's preview included.
    pub fn shown_settings(&self) -> Option<&oni_settings::PipelineSettings> {
        self.requests.requested_live.as_ref()
    }

    pub fn renders(&self) -> usize {
        self.renders
    }

    /// How many times a photo was opened in the editor.
    pub fn opens(&self) -> usize {
        self.opens
    }

    /// The store revision the canvas last showed.
    pub fn rendered(&self) -> u64 {
        self.requests.rendered
    }

    pub fn store(&self) -> &dyn Settings {
        self.store.as_ref()
    }

    /// `Diagnostics.witness`: what the window showed, written beside a block.
    pub fn witness(&self) -> String {
        match self.open_path() {
            Some(path) => format!("editing photo {}", crate::anonymity::token(path)),
            None => "the gallery".into(),
        }
    }

    pub fn open_path(&self) -> Option<&str> {
        self.open.as_deref()
    }

    /// The library column's height, which a check places a row in view against.
    pub fn tree_height(&self) -> f32 {
        self.tree.size.1
    }

    /// The thumbnails of the opened folder settled so far: held in the folder, drawn, or refused.
    pub fn loaded(&self) -> usize {
        self.painted.0
    }

    /// Whether the opened folder's read has landed whole.
    pub fn read_done(&self) -> bool {
        self.read_done
    }

    /// Whether a gesture's writes or moves are still running off the interface's thread.
    pub fn disk_busy(&self) -> bool {
        self.org.disk_busy.is_some()
    }

    /// Whether Prepare the Roll is shown.
    pub fn preparing(&self) -> bool {
        self.prepare.shown
    }

    /// The roll's frames shown on their own preview rather than their thumbnail.
    pub fn roll_previewed(&self) -> usize {
        self.prepare.previewed()
    }

    /// The frames whose full resolution the base step's loupe holds ready.
    pub fn roll_fulls(&self) -> Vec<String> {
        self.prepare.fulls.iter().map(|(p, _, _)| p.clone()).collect()
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

    /// The photos in the grid's order, which a tile's index counts in.
    pub fn order(&self) -> &[String] {
        &self.order
    }

    pub fn visible(&self) -> std::ops::Range<usize> {
        self.visible.clone()
    }

    /// The gallery grid's width and height, as Slint last laid it.
    pub fn grid(&self) -> (f32, f32) {
        self.grid
    }

    pub fn brush(&self) -> f32 {
        self.brush
    }

    /// The gallery's selection, which the editor's ⌘A completes.
    pub fn picked(&self) -> &HashSet<String> {
        &self.picks.selection
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
        self.compare.split
    }

    pub fn befores(&self) -> usize {
        self.compare.befores
    }

    pub fn phone(&self) -> Option<&Phone> {
        self.phone.as_ref()
    }

    /// `loadBar`: what the bar under the card shows loading, in every mode but export.
    pub fn load_bar(&self) -> Option<LoadBar> {
        if let Some(gesture) = &self.org.disk_busy {
            return Some(LoadBar::busy(gesture));
        }
        let (done, total) = self.creating;
        if done < total {
            return Some(LoadBar::counted(tr("Creating thumbnails"), done, total));
        }
        if !self.read_done && self.opened.is_some() {
            let waiting = self.rows.iter().filter(|e| e.fingerprint.starts_with(thumbnails::PENDING)).count();
            return Some(LoadBar::counted(tr("Reading the library"), self.rows.len() - waiting, self.rows.len()));
        }
        if let Some(m) = self.making_sheet {
            // Only the decode counts frames; writing, adopting and opening are opaque, so the bar goes busy.
            return Some(if m.step.is_empty() { LoadBar::counted(tr("Making the contact sheet"), m.done, m.total) } else { LoadBar::busy(tr(m.step)) });
        }
        if let Some((done, total)) = self.together.as_ref().map(|g| g.loaded()).filter(|(done, total)| done < total) {
            return Some(LoadBar::counted(tr("Opening the frames together"), done, total));
        }
        self.building_full.as_ref().map(|_| LoadBar::busy(tr("Loading full resolution")))
    }

    /// The open frame's place in its group, one-based, and the group's count.
    pub(super) fn place(&self) -> Option<(u16, u16)> {
        let open = self.open_path()?;
        let group = self.archive.as_ref()?.groups.iter().find(|g| g.entries.iter().any(|e| e.path == open))?;
        let at = group.entries.iter().position(|e| e.path == open)?;
        Some((at as u16 + 1, group.entries.len() as u16))
    }
}

/// A line of the load bar: what loads, and its count when the work has one.
#[derive(Clone, Debug, PartialEq)]
pub struct LoadBar {
    pub title: String,
    pub progress: Option<(usize, usize)>,
}

impl LoadBar {
    fn counted(title: &str, done: usize, total: usize) -> LoadBar {
        LoadBar { title: title.into(), progress: Some((done, total)) }
    }

    fn busy(title: &str) -> LoadBar {
        LoadBar { title: title.into(), progress: None }
    }

    /// The count on the glass, `3/38`; nothing when the work does not count.
    pub fn count(&self) -> String {
        self.progress.map(|(done, total)| format!("{done}/{total}")).unwrap_or_default()
    }

    /// The fraction done, below zero when the bar sweeps.
    pub fn fraction(&self) -> f32 {
        self.progress.map_or(-1.0, |(done, total)| done as f32 / total.max(1) as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::LoadBar;

    #[test]
    fn a_counted_load_reads_its_count_and_fraction_and_a_busy_one_sweeps() {
        let counted = LoadBar::counted("Making the contact sheet", 3, 8);
        assert_eq!((counted.count(), counted.fraction()), ("3/8".to_string(), 0.375));
        let busy = LoadBar::busy("Writing the file");
        // Twin: a busy bar never reads as a bar stalled at nothing done.
        assert_eq!((busy.count(), busy.fraction()), (String::new(), -1.0));
        assert_eq!(LoadBar::counted("Reading the library", 0, 0).fraction(), 0.0, "no work is not a sweep");
    }
}
