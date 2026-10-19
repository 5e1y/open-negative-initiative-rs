//! The year on film: each gesture noted in the journal as Swift notes it, and "Show My Year on Film"
//! read back off it, the title then the deck, Esc leaving.

use super::{Ctl, image, with_shell};
use crate::wrapped::{Card, cards};
use crate::{Wrapped, WrappedFace};
use oni_library::journal::{Journal, Kind};
use oni_library::wrapped::{WrappedYear, local_year, today};
use oni_settings::{Medal, PipelineSettings};
use slint::{ComponentHandle, Image, ModelRc, SharedString, VecModel};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// `@AppStorage("wrappedShownYear")`: December offers the year once.
pub const SHOWN_YEAR_KEY: &str = "wrappedShownYear";
/// A full sample year instead of the journal's, to look at the summary before living one.
pub const SAMPLE: &str = "ONI_WRAPPED_SAMPLE";

#[derive(Default)]
pub struct Year {
    /// `JournalEdits.last`: each photo's settings at its last settled edit.
    last: HashMap<String, PipelineSettings>,
    /// The summary while it shows: its cards, the one on top, and whether the deck is dealt.
    deck: Option<(Vec<Card>, usize, bool)>,
    /// The launch's offer, waiting for the opened folder's rows as Swift's waits for `fillOpened`.
    offer_due: bool,
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
            let Some(path) = s.open.clone() else { return };
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
        w.on_forward(move || p.wrapped_forward());
        let p = me.clone();
        w.on_back(move || p.wrapped_turn(-1));
        let p = me.clone();
        w.on_close(move || p.close_wrapped());
    }

    /// `offerWrapped()`: in December, once a year, never under a bench.
    /// `offerWrapped()` once the opened folder is read, at once when none is being read.
    pub fn offer_wrapped_when_read(self: &Rc<Self>) {
        let reading = {
            let s = self.state.borrow();
            s.opened.is_some() && !s.read_done
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

    pub fn offer_wrapped(self: &Rc<Self>) {
        let (year, month) = today();
        let shown = self.state.borrow().preferences.value(SHOWN_YEAR_KEY).and_then(serde_json::Value::as_i64).unwrap_or(0);
        if month != 12 || shown >= i64::from(year) {
            return;
        }
        self.remember(SHOWN_YEAR_KEY, year.into());
        self.show_wrapped(year);
    }

    /// `showWrapped(year:)`: the journal read off this thread, the ONI Trash's photos left out.
    pub(super) fn show_wrapped(self: &Rc<Self>, year: i32) {
        if self.state.borrow().year.deck.is_some() {
            return;
        }
        if std::env::var_os(SAMPLE).is_some() {
            let order = self.state.borrow().order.clone();
            return self.present_wrapped(WrappedYear::sample(year, &order));
        }
        let journal = self.journal();
        let hidden: HashSet<String> = {
            let s = self.state.borrow();
            s.index.as_ref().map(|i| oni_library::index::trashed(&i.all()).into_iter().map(|e| e.path).collect()).unwrap_or_default()
        };
        std::thread::spawn(move || {
            let (records, names) = journal.read();
            let built = WrappedYear::build(&records, &names, year, &hidden, local_year);
            with_shell(move |c| c.present_wrapped(built));
        });
    }

    fn present_wrapped(self: &Rc<Self>, year: WrappedYear) {
        let Some(w) = self.window() else { return };
        let deck = cards(&year);
        let faces: Vec<WrappedFace> = deck
            .iter()
            .map(|c| WrappedFace {
                eyebrow: c.eyebrow.to_uppercase().into(),
                figure: c.figure.clone().unwrap_or_default().into(),
                caption: c.caption.clone().into(),
                photos: ModelRc::new(VecModel::from(vec![Image::default(); c.photos.len()])),
            })
            .collect();
        let g = w.global::<Wrapped>();
        g.set_year(year.year.to_string().into());
        g.set_index(0);
        g.set_dealt(false);
        g.set_cards(ModelRc::new(VecModel::from(faces)));
        g.set_showing(g.get_showing() + 1);
        g.set_shown(true);
        let wanted: Vec<(usize, usize, String)> = {
            let s = self.state.borrow();
            deck.iter().enumerate().flat_map(|(i, c)| c.photos.iter().enumerate().filter_map(move |(j, p)| Some((i, j, p.clone())))).filter_map(|(i, j, p)| s.fingerprints.get(&p).map(|f| (i, j, f.clone()))).collect()
        };
        self.state.borrow_mut().year.deck = Some((deck, 0, false));
        self.publish_bar();
        // `Thumbnails.alreadyRendered`: only the thumbnails on disk, never a render for the summary.
        let (support, screen) = {
            let s = self.state.borrow();
            (s.support.clone(), s.screen)
        };
        std::thread::spawn(move || {
            for (card, slot, fingerprint) in wanted {
                let Some((rgba, width, height)) = crate::thumbnails::load(&support, &fingerprint, screen) else { continue };
                with_shell(move |c| c.wrapped_photo(card, slot, image(&rgba, width, height)));
            }
        });
    }

    fn wrapped_photo(&self, card: usize, slot: usize, picture: Image) {
        let Some(w) = self.window() else { return };
        let g = w.global::<Wrapped>();
        if !g.get_shown() {
            return;
        }
        use slint::Model;
        if let Some(face) = g.get_cards().row_data(card) {
            face.photos.set_row_data(slot, picture);
        }
    }

    fn wrapped_forward(&self) {
        let dealt = self.state.borrow().year.deck.as_ref().is_some_and(|d| d.2);
        if dealt {
            return self.wrapped_turn(1);
        }
        if let Some(deck) = self.state.borrow_mut().year.deck.as_mut() {
            deck.2 = true;
        }
        if let Some(w) = self.window() {
            w.global::<Wrapped>().set_dealt(true);
        }
    }

    /// `turn(by:)`: past the last card the summary closes.
    fn wrapped_turn(&self, by: i32) {
        let Some((count, index)) = self.state.borrow().year.deck.as_ref().map(|d| (d.0.len() as i32, d.1 as i32)) else { return };
        let next = index + by;
        if next < 0 {
            return;
        }
        if next >= count {
            return self.close_wrapped();
        }
        if let Some(deck) = self.state.borrow_mut().year.deck.as_mut() {
            deck.1 = next as usize;
        }
        if let Some(w) = self.window() {
            w.global::<Wrapped>().set_index(next);
        }
    }

    pub(super) fn close_wrapped(&self) {
        self.state.borrow_mut().year.deck = None;
        if let Some(w) = self.window() {
            let g = w.global::<Wrapped>();
            g.set_shown(false);
            g.set_cards(ModelRc::default());
            g.set_year(SharedString::default());
            w.invoke_refocus();
        }
        self.publish_bar();
    }
}
