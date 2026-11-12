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

    /// A full resolution held, whichever frame it belongs to.
    pub fn full_resident(&self) -> bool {
        self.full.is_some()
    }

    /// The settings the canvas was last asked to show, a hovered preset's preview included.
    pub fn shown_settings(&self) -> Option<&oni_settings::PipelineSettings> {
        self.requests.requested_live.as_ref()
    }

    /// Every preview kept in memory let go, as a launch finds them: the next opening decodes.
    pub fn forget_previews(&self) {
        self.previews.lock().expect("previews").clear();
    }

    pub fn renders(&self) -> usize {
        self.renders
    }

    /// How many times a photo was opened in the editor.
    pub fn opens(&self) -> usize {
        self.opens
    }

    /// The picture's histogram the plots trace, read by the checks.
    pub fn histogram(&self) -> Option<&oni_measure::histogram::Histogram> {
        self.requests.histogram.as_ref()
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

    /// Whether a decode of `path` under `reading` is the open frame's: any other is superseded.
    pub(super) fn wants(&self, path: &str, reading: &render::Reading) -> bool {
        self.open_path() == Some(path) && self.reading.as_ref() == Some(reading)
    }

    /// The library column's height, which a check places a row in view against.
    pub fn tree_height(&self) -> f32 {
        self.tree.size.1
    }

    /// The thumbnails of the opened folder settled so far: held in the folder, drawn, or refused.
    pub fn loaded(&self) -> usize {
        self.painted.0
    }

    /// Every thumbnail drawn and landed since launch, a redraw included.
    pub fn thumbnails_landed(&self) -> usize {
        self.org.thumbnails_landed
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

    /// The frame on the card's top layer, the one it shows once its dissolve ends.
    pub fn roll_card(&self) -> Option<&str> {
        self.prepare.card_shown()
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

    /// Under a rating filter, every photo it shows, in the gallery's order: the set it was turned on
    /// to find, which Select All picks whole; none in the ONI Trash, which no filter cuts.
    pub(super) fn filter_shows(&self) -> Option<Vec<String>> {
        (self.filter.is_some() && !self.trash_shown()).then(|| self.order.clone())
    }

    /// The thumbnails in memory dropped, as a long scroll drops them, so a check sees a tile read
    /// back from the folder.
    pub fn forget_tiles(&mut self) {
        self.thumbs.clear();
        self.asked.clear();
    }

    /// How many thumbnails memory holds, which only the rows and cells near a view ask.
    pub fn tiles_held(&self) -> usize {
        self.thumbs.len()
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

    /// Whether a frame's dust patches are being generated.
    pub fn healing(&self) -> bool {
        !self.dust.healing.is_empty()
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
        if let Some(bar) = balance_bar(self.batch, std::time::Instant::now()) {
            return Some(bar);
        }
        let (done, total) = self.creating;
        if done < total {
            return Some(LoadBar::counted(tr("Creating thumbnails"), done, total));
        }
        if !self.read_done && self.opened.is_some() {
            let waiting = self.rows.iter().filter(|e| e.fingerprint.starts_with(thumbnails::PENDING)).count();
            return Some(LoadBar::counted(tr("Reading the library"), self.rows.len() - waiting, self.rows.len()));
        }
        if let Some((done, total)) = self.together.as_ref().map(|g| g.loaded()).filter(|(done, total)| done < total) {
            return Some(LoadBar::counted(tr("Opening the frames together"), done, total));
        }
        self.building_full.as_ref().map(|_| LoadBar::busy(tr("Loading full resolution")))
    }

    /// `labFrames`: the open frame's group, which the arrows walk: its project on the Projects tab,
    /// read off its rolls' bands laid, else its roll or day.
    pub(super) fn lab_frames(&self) -> std::borrow::Cow<'_, [oni_library::Entry]> {
        let Some(open) = self.open_path() else { return Default::default() };
        let project = self.org.editing_project.filter(|_| self.org.projects_surface).map(|id| id.to_string());
        let bands = self.org.bands.iter().filter(|b| project.as_ref() == Some(&b.key));
        let frames: Vec<oni_library::Entry> = bands.flat_map(|b| b.entries.iter().cloned()).collect();
        if !frames.is_empty() {
            return frames.into();
        }
        let group = self.archive.as_ref().and_then(|a| a.groups.iter().find(|g| g.entries.iter().any(|e| e.path == open)));
        group.map_or(&[][..], |g| &g.entries).into()
    }

    /// The frame the editor goes on to once `leaving` goes to the ONI Trash with the open one.
    pub(super) fn after_trash(&self, leaving: &[String]) -> Option<String> {
        let frames = self.lab_frames();
        let paths: Vec<&str> = frames.iter().map(|e| e.path.as_str()).collect();
        after_trash(&paths, self.open_path()?, leaving)
    }

    /// The open frame's place in its group, one-based, and the group's count.
    pub(super) fn place(&self) -> Option<(usize, usize)> {
        let open = self.open_path()?;
        let frames = self.lab_frames();
        let at = frames.iter().position(|e| e.path == open)?;
        Some((at + 1, frames.len()))
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

/// The balance key's run in the status bar, once it has lasted past `BALANCE_BAR_AFTER`.
fn balance_bar(batch: Option<super::rolls::Batch>, now: std::time::Instant) -> Option<LoadBar> {
    let b = batch.filter(|b| now.duration_since(b.started) >= super::rolls::BALANCE_BAR_AFTER)?;
    Some(LoadBar::counted(tr("Auto white balance"), b.done, b.total))
}

/// The first frame of the roll after `open` that stays, else the last before it; none while `open`
/// itself stays.
fn after_trash(roll: &[&str], open: &str, leaving: &[String]) -> Option<String> {
    let at = roll.iter().position(|p| *p == open).filter(|_| leaving.iter().any(|l| l == open))?;
    let stays = |p: &&&str| !leaving.iter().any(|l| l == **p);
    roll[at + 1..].iter().find(stays).or_else(|| roll[..at].iter().rev().find(stays)).map(|p| (*p).to_owned())
}

#[cfg(test)]
mod tests {
    use super::{LoadBar, after_trash, balance_bar};
    use crate::app::rolls::{BALANCE_BAR_AFTER, Batch};
    use std::time::{Duration, Instant};

    #[test]
    fn a_balance_run_counts_in_the_status_bar_once_it_outlasts_its_delay() {
        let started = Instant::now();
        let run = Some(Batch { done: 2, total: 5, started });
        let late = balance_bar(run, started + BALANCE_BAR_AFTER + Duration::from_millis(1)).expect("a long run shows");
        assert_eq!((late.count(), late.fraction()), ("2/5".to_string(), 0.4));
        // Twin: a quick run stays off the bar, and no run never shows.
        assert!(balance_bar(run, started + BALANCE_BAR_AFTER - Duration::from_millis(1)).is_none());
        assert!(balance_bar(None, started + Duration::from_secs(5)).is_none());
    }

    #[test]
    fn a_frame_trashed_in_the_editor_hands_on_to_the_next_else_the_one_before() {
        let roll = ["a", "b", "c", "d"];
        let gone = |p: &[&str]| p.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(after_trash(&roll, "b", &gone(&["b"])).as_deref(), Some("c"));
        assert_eq!(after_trash(&roll, "b", &gone(&["b", "c"])).as_deref(), Some("d"), "past the others leaving with it");
        assert_eq!(after_trash(&roll, "d", &gone(&["d"])).as_deref(), Some("c"), "the roll's end: the one before");
        assert_eq!(after_trash(&roll, "c", &gone(&["c", "d", "b"])).as_deref(), Some("a"));
        // Twin: the whole roll gone, or the open frame staying, leaves none.
        assert_eq!(after_trash(&roll, "a", &gone(&roll)), None);
        assert_eq!(after_trash(&roll, "a", &gone(&["b"])), None);
    }

    #[test]
    fn a_counted_load_reads_its_count_and_fraction_and_a_busy_one_sweeps() {
        let counted = LoadBar::counted("Opening the frames together", 3, 8);
        assert_eq!((counted.count(), counted.fraction()), ("3/8".to_string(), 0.375));
        let busy = LoadBar::busy("Writing the file");
        // Twin: a busy bar never reads as a bar stalled at nothing done.
        assert_eq!((busy.count(), busy.fraction()), (String::new(), -1.0));
        assert_eq!(LoadBar::counted("Reading the library", 0, 0).fraction(), 0.0, "no work is not a sweep");
    }
}
