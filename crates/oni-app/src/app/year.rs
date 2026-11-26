//! The year on film: each gesture noted in the journal, and "Show My Year on Film" read back off the
//! journal, the library and its thumbnails as a strip of scenes, each one a card that can be saved.

use super::Ctl;
use crate::Wrapped;
use crate::wrapped::Scene;
use oni_library::journal::{Journal, Kind};
use oni_library::wrapped::today;
use oni_settings::{Medal, PipelineSettings};
use slint::ComponentHandle;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

mod cards;
mod showing;

/// `@AppStorage("wrappedShownYear")`: December offers the year once.
pub const SHOWN_YEAR_KEY: &str = "wrappedShownYear";
/// A full sample year instead of the journal's, to look at the summary before living one.
pub const SAMPLE: &str = "ONI_WRAPPED_SAMPLE";
/// What a posted card is tagged with, and the account it names: on every card, in every language.
pub const HASHTAG: &str = "#madewithoni";
pub const ACCOUNT: &str = "@open.negative.initiative";
/// The app's name beside its mark, the same in every language.
pub(super) const NAME: &str = "ONI";
/// A print's long side, enough for a frame as large as a tall window draws it.
pub(super) const PRINT_SIDE: usize = 1024;
/// A scene showing more photos than this shows them small: their thumbnails are enough.
pub(super) const PRINTS_A_SCENE: usize = 4;

/// Whether December's offer is still to make: in December, for a year not shown yet.
pub fn offer_due((year, month): (i32, u32), shown: i64) -> bool {
    month == 12 && shown < i64::from(year)
}

/// The year "Show My Year on Film" tells: the one under way, or in January the one just ended.
pub fn told_year((year, month): (i32, u32)) -> i32 {
    if month == 1 { year - 1 } else { year }
}

/// The year the gallery's entry tells, through December and the January after it; no other month has one.
pub fn season(today: (i32, u32)) -> Option<i32> {
    matches!(today.1, 12 | 1).then(|| told_year(today))
}

/// The season's year as the entry's seal shows it, empty out of the season.
fn season_word(today: (i32, u32)) -> String {
    season(today).map(|year| year.to_string()).unwrap_or_default()
}

#[derive(Default)]
pub struct Year {
    /// `JournalEdits.last`: each photo's settings at its last settled edit.
    last: HashMap<String, PipelineSettings>,
    /// The summary while it shows: the year it tells, its scenes and the one in the gate.
    story: Option<Story>,
    /// A showing asked, its year still being read off the disk.
    reading: bool,
    /// Counts the showings and their ends: a photo landing for a summary since closed is dropped.
    told: Arc<AtomicUsize>,
    /// The launch's offer, waiting for the opened folder's rows as Swift's waits for `fillOpened`.
    offer_due: bool,
}

/// The summary while it shows: the year it tells, its scenes, the one in the gate, and whether the
/// story is over and its cards laid out to share.
struct Story {
    year: i32,
    scenes: Vec<Scene>,
    index: usize,
    sharing: bool,
}

fn medal_rank(medal: Medal) -> u16 {
    match medal {
        Medal::None => 0,
        Medal::Bronze => 1,
        Medal::Silver => 2,
        Medal::Gold => 3,
    }
}

/// The year's journal in `support`; a bench drags the owner's real roll, so its gestures go to a
/// scratch journal instead, as Swift's checks write under a stand-in folder.
pub(super) fn journal_in(support: &std::path::Path) -> std::sync::Arc<Journal> {
    if crate::bench::requested().is_some() {
        return Journal::in_support(&std::env::temp_dir().join(format!("oni-bench-journal-{}", std::process::id())));
    }
    Journal::in_support(support)
}

impl Ctl {
    fn journal(&self) -> std::sync::Arc<Journal> {
        journal_in(&self.state.borrow().support)
    }

    fn note(&self, kind: Kind, subject: Option<&str>, detail: u16, value: u32, name: Option<&str>) {
        self.journal().note(kind, subject, detail, value, name, oni_settings::Date::now());
    }

    /// `load(_:)`: the photo opened, its settings the ground its first edit is read against.
    pub(super) fn journal_opened(&self, path: &str) {
        self.note(Kind::Opened, Some(path), 0, 0, None);
        let settings = self.state.borrow().store.settings().clone();
        self.state.borrow_mut().year.last.insert(path.to_string(), settings);
    }

    /// `JournalEdits.settled`: one record per settings row the gesture moved, never one per notch.
    pub(super) fn journal_settled(&self) {
        let (path, settings, before) = {
            let s = self.state.borrow();
            let Some(path) = s.open_path().map(str::to_string) else { return };
            let settings = s.store.settings().clone();
            let before = s.year.last.get(&path).cloned();
            (path, settings, before)
        };
        if let Some(before) = before.filter(|b| *b != settings) {
            for (rank, row) in oni_core::parameters::all().iter().enumerate() {
                let mut probe = before.clone();
                (row.carry)(&settings, &mut probe);
                if probe != before {
                    self.note(Kind::Edited, Some(&path), rank as u16, 0, None);
                }
            }
        }
        self.state.borrow_mut().year.last.insert(path, settings);
    }

    pub(super) fn journal_medal(&self, paths: &[String], medal: Medal) {
        for path in paths {
            self.note(Kind::Medal, Some(path), medal_rank(medal), 0, None);
        }
    }

    pub(super) fn journal_preset(&self, paths: &[String], name: &str) {
        for path in paths {
            self.note(Kind::Preset, Some(path), 0, 0, Some(name));
        }
    }

    /// `.pasted` and `.rolled` count the photos the gesture reached.
    pub(super) fn journal_count(&self, kind: Kind, count: usize) {
        self.note(kind, None, 0, count as u32, None);
    }

    pub(super) fn journal_balanced(&self) {
        self.note(Kind::Balanced, None, 0, 0, None);
    }

    pub(super) fn wire_wrapped(me: &Rc<Ctl>, window: &crate::MainWindow) {
        let w = window.global::<Wrapped>();
        let p = me.clone();
        w.on_forward(move || p.wrapped_turn(1));
        let p = me.clone();
        w.on_back(move || p.wrapped_turn(-1));
        let p = me.clone();
        w.on_close(move || p.close_wrapped());
        let p = me.clone();
        w.on_open(move || p.act(crate::Act::Wrapped));
        w.set_season(season_word(today()).into());
        let p = me.clone();
        w.on_save_all(move || p.save_wrapped(None));
        let p = me.clone();
        w.on_save_card(move |scene| p.save_wrapped(Some(scene.max(0) as usize)));
        let p = me.clone();
        w.on_copy_card(move |scene| p.copy_wrapped(scene.max(0) as usize));
        w.on_counted(|n| crate::wrapped::figure(n.max(0) as usize).into());
    }

    /// The launch's offer once the opened folder is read, at once when none is being read.
    pub fn offer_wrapped_when_read(self: &Rc<Self>) {
        let reading = {
            let s = self.state.borrow();
            s.opened().is_some() && !s.read_done()
        };
        if reading {
            self.state.borrow_mut().year.offer_due = true;
        } else {
            self.offer_wrapped();
        }
    }

    /// The launch's offer made now if it waited for the rows just read.
    pub(super) fn wrapped_after_read(self: &Rc<Self>) {
        let due = std::mem::take(&mut self.state.borrow_mut().year.offer_due);
        if due {
            self.offer_wrapped();
        }
    }

    /// The gallery's entry for `today`: the year it tells in its season, none out of it.
    pub fn wrapped_season(&self, today: (i32, u32)) {
        let Some(w) = self.window() else { return };
        w.global::<Wrapped>().set_season(season_word(today).into());
    }

    pub(super) fn shown_year(&self) -> i64 {
        self.state.borrow().preferences.value(SHOWN_YEAR_KEY).and_then(serde_json::Value::as_i64).unwrap_or(0)
    }

    /// In December, once a year, never under a bench, and never at the launch that shows the welcome,
    /// which gives the year itself.
    pub fn offer_wrapped(self: &Rc<Self>) {
        if offer_due(today(), self.shown_year()) && !self.welcome_due() {
            self.show_wrapped(today().0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn january_tells_the_year_just_ended_and_every_other_month_its_own() {
        assert_eq!(told_year((2027, 1)), 2026);
        assert_eq!((told_year((2026, 12)), told_year((2027, 2))), (2026, 2027));
        // Twin: January's own year is not the one told.
        assert_ne!(told_year((2027, 1)), 2027);
    }

    #[test]
    fn december_offers_a_year_once_and_no_other_month_does() {
        assert!(offer_due((2026, 12), 0) && offer_due((2026, 12), 2025));
        assert!(!offer_due((2026, 12), 2026), "shown already");
        // Twin: the same year not yet shown is not offered in November.
        assert!(!offer_due((2026, 11), 0));
    }

    #[test]
    fn the_gallerys_entry_lasts_december_and_january_and_tells_one_year() {
        assert_eq!((season((2026, 12)), season((2027, 1))), (Some(2026), Some(2026)));
        // Twin: the months on either side have none.
        assert_eq!((season((2026, 11)), season((2027, 2))), (None, None));
    }
}
