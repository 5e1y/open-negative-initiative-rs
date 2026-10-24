//! What another app changes on the disk shows by itself: the opened folder and the listed ones
//! watched, the opened folder looked at again when the window comes back, and Refresh saying what moved.

use super::*;
use oni_library::watching::{self, Listing, Pace, Touched, Watch};
use oni_i18n::trn;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// The navigator's listed folders watched at most, the deepest unfolded ones dropped first.
const FLAT_WATCHES: usize = 32;

pub(in crate::app) struct Fresh {
    /// Off in a check that proves another path alone, as the window's return.
    pub on: bool,
    watch: Option<Watch>,
    watched: (Vec<String>, Vec<String>),
    /// A change seen while the opened folder's read ran, which reads it again once that one lands.
    stale: bool,
    /// The photos held when Refresh was asked, which its answer counts against.
    refreshing: Option<HashSet<String>>,
    /// The opened folder's listing when the window went to the back, compared when it comes back.
    away: Arc<Mutex<Option<(String, Listing)>>>,
    /// A look at the opened folder under way, so a disk that hangs holds one thread, never one per return.
    looking: Arc<AtomicBool>,
    noting: Arc<AtomicBool>,
    returns: usize,
}

impl Default for Fresh {
    fn default() -> Fresh {
        Fresh {
            on: true,
            watch: None,
            watched: Default::default(),
            stale: false,
            refreshing: None,
            away: Arc::default(),
            looking: Arc::default(),
            noting: Arc::default(),
            returns: 0,
        }
    }
}

impl Ctl {
    /// The watches made to follow the opened folder, whole, and the listed folders, a level down.
    pub(in crate::app) fn rewatch(&self) {
        let mut s = self.state.borrow_mut();
        if !s.nav.fresh.on {
            return;
        }
        let deep: Vec<String> = s.opened.iter().filter(|_| !s.org.trash_open).cloned().collect();
        let model = &s.nav.model;
        let mut flat: Vec<String> = model.base.iter().cloned().collect();
        flat.extend(s.nav.rows.iter().filter(|r| model.unfolded.contains(*r)).cloned());
        flat.retain(|f| !deep.iter().any(|d| holds(d, f)));
        flat.sort();
        flat.dedup();
        flat.sort_by_key(|f| f.matches('/').count());
        flat.truncate(FLAT_WATCHES);
        if s.nav.fresh.watched == (deep.clone(), flat.clone()) {
            return;
        }
        let computer = s.nav.computer.clone();
        let fresh = &mut s.nav.fresh;
        fresh.watched = (deep.clone(), flat.clone());
        let watch = fresh.watch.get_or_insert_with(|| Watch::start(computer, Pace::APP, |touched| with_shell(move |c| c.disk_changed(touched))));
        watch.set(deep, flat);
    }

    /// Watching on or off, for a check that proves the window's return alone.
    pub fn set_watching(&self, on: bool) {
        {
            let mut s = self.state.borrow_mut();
            s.nav.fresh.on = on;
            s.nav.fresh.watch = None;
            s.nav.fresh.watched = Default::default();
        }
        self.rewatch();
    }

    /// A burst told by the watches: the opened folder read again, the listed folders listed again.
    fn disk_changed(self: &Rc<Self>, touched: Touched) {
        let opened = {
            let mut s = self.state.borrow_mut();
            for folder in &touched.flat {
                s.nav.model.list(folder, true);
            }
            let opened = s.opened.clone().filter(|o| touched.deep.iter().any(|d| paths::same(d, o)));
            if opened.is_some() && !s.read_done {
                s.nav.fresh.stale = true;
                None
            } else {
                opened
            }
        };
        self.list_wanted();
        if opened.is_some() {
            self.read_again();
            self.note_listing();
        }
    }

    /// The opened folder's read landed whole: one more if the disk moved meanwhile, Refresh's answer.
    pub(super) fn fresh_read(self: &Rc<Self>) {
        let (again, said) = {
            let mut s = self.state.borrow_mut();
            let again = std::mem::take(&mut s.nav.fresh.stale);
            let said = if again { None } else { s.nav.fresh.refreshing.take().map(|before| s.refreshed(&before)) };
            (again, said)
        };
        if again {
            return self.read_again();
        }
        if let Some(said) = said {
            self.told(tr("Refresh"), &said);
        }
    }

    /// `Refresh`, ⌘R: the opened folder and every favourite read again, every listing listed again;
    /// once the opened folder lands, what moved is said.
    pub(in crate::app) fn refresh(self: &Rc<Self>) {
        let opened = {
            let mut s = self.state.borrow_mut();
            let held: HashSet<String> = s.rows.iter().map(|e| e.path.clone()).collect();
            s.nav.fresh.refreshing = s.opened.as_ref().map(|_| held);
            s.nav.surveys.areas.clear();
            s.nav.tried.clear();
            let listed: Vec<String> = s.nav.model.listings.keys().cloned().collect();
            for folder in listed {
                s.nav.model.list(&folder, true);
            }
            s.nav.model.retry();
            s.opened.clone()
        };
        self.list_wanted();
        if opened.is_some() {
            self.read_again();
            self.note_listing();
        } else {
            self.survey_favourites();
            self.told(tr("Refresh"), tr("The folders were read again."));
        }
    }

    /// The window to the back or to the front: on its return the opened folder is listed again off
    /// this thread, read again when a photo or a folder moved, else its photos only looked at.
    pub fn focus_changed(self: &Rc<Self>, focused: bool) {
        if !focused {
            return self.note_listing();
        }
        let (opened, away, looking) = {
            let s = self.state.borrow();
            let Some(opened) = s.opened.clone().filter(|_| s.read_done) else { return };
            (opened, s.nav.fresh.away.clone(), s.nav.fresh.looking.clone())
        };
        let seen = away.lock().expect("away").take().filter(|(folder, _)| *folder == opened);
        let Some((_, before)) = seen else { return self.probe_presence() };
        if looking.swap(true, Ordering::AcqRel) {
            return;
        }
        let _ = std::thread::Builder::new().name("returning".into()).spawn(move || {
            let moved = watching::listing(&opened).is_none_or(|now| now != before);
            looking.store(false, Ordering::Release);
            with_shell(move |c| c.returned(opened, moved));
        });
    }

    fn returned(self: &Rc<Self>, folder: String, moved: bool) {
        let (same, reading) = {
            let mut s = self.state.borrow_mut();
            s.nav.fresh.returns += 1;
            (s.opened.as_deref() == Some(folder.as_str()), !s.read_done)
        };
        match (same, moved, reading) {
            (false, ..) => {}
            (true, true, true) => self.state.borrow_mut().nav.fresh.stale = true,
            (true, true, false) => self.read_again(),
            (true, false, _) => self.probe_presence(),
        }
    }

    /// The opened folder's listing kept off this thread, the one the window's return compares with.
    fn note_listing(&self) {
        let (opened, away, noting) = {
            let s = self.state.borrow();
            let Some(opened) = s.opened.clone() else { return };
            (opened, s.nav.fresh.away.clone(), s.nav.fresh.noting.clone())
        };
        if noting.swap(true, Ordering::AcqRel) {
            return;
        }
        let _ = std::thread::Builder::new().name("listing".into()).spawn(move || {
            let listing = watching::listing(&opened);
            *away.lock().expect("away") = listing.map(|l| (opened, l));
            noting.store(false, Ordering::Release);
        });
    }
}

impl State {
    /// Whether the opened folder's listing is kept for the window's return.
    pub fn listing_kept(&self) -> bool {
        let fresh = &self.nav.fresh;
        !fresh.noting.load(Ordering::Acquire) && fresh.away.lock().expect("away").is_some()
    }

    /// The window's returns whose look at the opened folder landed.
    pub fn returns(&self) -> usize {
        self.nav.fresh.returns
    }

    /// What a read changed against the photos held before it: added, gone, out of reach.
    fn refreshed(&self, before: &HashSet<String>) -> String {
        let now: HashSet<&String> = self.rows.iter().map(|e| &e.path).collect();
        let added = now.iter().filter(|p| !before.contains(**p)).count() as u64;
        let gone = before.iter().filter(|p| !now.contains(p)).count() as u64;
        let away = self.rows.iter().filter(|e| !self.presence(&e.path).is_present()).count() as u64;
        let mut said: Vec<String> = Vec::new();
        if added > 0 {
            said.push(trn("{n} photo added.", "{n} photos added.", added));
        }
        if gone > 0 {
            said.push(trn("{n} photo no longer in the folder.", "{n} photos no longer in the folder.", gone));
        }
        if said.is_empty() {
            said.push(tr("Nothing changed.").to_string());
        }
        if away > 0 {
            said.push(trn("{n} photo cannot be found.", "{n} photos cannot be found.", away));
        }
        said.join(" ")
    }
}
