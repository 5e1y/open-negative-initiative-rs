//! The roll gestures the core already carries, reached from the window: Reset to Default…, Ungroup,
//! the Measured Roll block's Join and Roll of the N, and Auto White Balance over several frames
//! with its Each or Average choice and its progress on the key.

use super::{Ctl, Pending, with_shell};
use crate::{App, Choice, PanelState};
use oni_core::balance::Strategy;
use oni_library::roll_membership::{self, ResetReach};
use oni_measure::choice;
use oni_i18n::{tr, trn};
use oni_settings::PipelineSettings;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Frames balanced at once: each holds a preview decode, about 100 MB, until its copy is made.
const BALANCERS: usize = 3;

/// What the Measured Roll block shows: `RollScope`.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct RollScope {
    pub measured: Option<usize>,
    pub unmeasured: Vec<String>,
    pub reference: bool,
}

/// A run of the balance key under way.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Batch {
    pub done: usize,
    pub total: usize,
}

impl super::State {
    /// `rollMembers`: the targets carrying a roll, in the gallery only.
    pub(super) fn roll_members(&self, gallery: bool) -> Vec<String> {
        if !gallery {
            return Vec::new();
        }
        let targets: HashSet<String> = self.preset_targets().into_iter().collect();
        self.rows.iter().filter(|e| e.film.is_some() && targets.contains(&e.path)).map(|e| e.path.clone()).collect()
    }

    /// `rollScope`: the frames measured with the open frame's roll, and those of its folder not.
    pub(super) fn roll_scope(&mut self) -> RollScope {
        let Some(id) = self.store.settings().roll_reference.as_ref().map(|r| r.id) else { return RollScope::default() };
        let Some(store) = self.store.editor() else { return RollScope { reference: true, ..RollScope::default() } };
        let members = store.measured_with(id);
        let open = self.open.clone();
        let folder: Vec<String> = self
            .org
            .bands
            .iter()
            .find(|b| b.entries.iter().any(|e| Some(&e.path) == open.as_ref()))
            .map(|b| b.entries.iter().map(|e| e.path.clone()).collect())
            .unwrap_or_default();
        RollScope { reference: true, measured: Some(members.len()), unmeasured: folder.into_iter().filter(|p| !members.contains(p)).collect() }
    }
}

impl Ctl {
    pub(super) fn wire_rolls(me: &Rc<Ctl>, window: &crate::MainWindow) {
        let panel = window.global::<PanelState>();
        let p = me.clone();
        panel.on_join_roll(move || p.join_roll());
        let p = me.clone();
        panel.on_make_roll(move || p.make_roll_of_measured());
        let p = me.clone();
        window.global::<Choice>().on_chosen(move |i| p.balance_chosen(usize::try_from(i).ok()));
    }

    /// The Measured Roll block and the balance key's note, from the open frame and the targets.
    pub(super) fn publish_rolls(&self) {
        let Some(w) = self.window() else { return };
        let (scope, count, open_alone) = {
            let mut s = self.state.borrow_mut();
            let targets = s.balance_targets();
            let open_alone = targets.len() == 1 && s.open.as_ref() == targets.first();
            (s.roll_scope(), targets.len(), open_alone)
        };
        let panel = w.global::<PanelState>();
        panel.set_roll_state(match (scope.reference, scope.measured) {
            (false, _) => 0,
            (true, Some(_)) => 1,
            (true, None) => 2,
        });
        panel.set_roll_measured(scope.measured.unwrap_or(0) as i32);
        panel.set_roll_unmeasured(scope.unmeasured.len() as i32);
        // Stated before the click, while the target can still change.
        let note = if !choice::needs_run(count, open_alone) || count == 0 {
            String::new()
        } else if count > 1 {
            tr!("Acts on {count} photos.", count = count)
        } else {
            tr("Acts on the selected photo, not the one on screen.").to_string()
        };
        panel.set_balance_note(note.into());
    }

    /// `applyRollToUnmeasured`: the roll's measurement and settings on the folder's frames it missed.
    fn join_roll(self: &Rc<Self>) {
        let paths = self.state.borrow_mut().roll_scope().unmeasured;
        if paths.is_empty() {
            return;
        }
        self.gesture(&paths, |store| store.join_roll(&paths));
    }

    /// `makeRollOfMeasured`: the frames measured together made a roll of their own.
    fn make_roll_of_measured(self: &Rc<Self>) {
        let all: HashSet<String> = {
            let mut s = self.state.borrow_mut();
            let Some(id) = s.store.settings().roll_reference.as_ref().map(|r| r.id) else { return };
            let open = s.open.clone();
            let Some(store) = s.store.editor() else { return };
            let mut all: HashSet<String> = store.measured_with(id).into_iter().collect();
            all.extend(open);
            all
        };
        self.new_roll_from(&all);
        if let Some(store) = self.state.borrow_mut().store.editor() {
            store.forget_measured();
        }
        self.publish_bar();
    }

    /// `applyFilm(nil, to: rollMembers, label: "Ungroup")`.
    pub(super) fn ungroup(self: &Rc<Self>) {
        let gallery = self.window().is_some_and(|w| w.global::<App>().get_gallery());
        let members = self.state.borrow().roll_members(gallery);
        if members.is_empty() {
            return;
        }
        self.gesture(&members, |store| store.set_film(&members, None, tr("Ungroup")));
    }

    // MARK: - Reset to Default

    /// `askResetToDefault`: the reach read off this thread, then a confirm naming what goes.
    pub(super) fn ask_reset_to_default(self: &Rc<Self>) {
        let (picked, library) = {
            let s = self.state.borrow();
            (s.preset_targets(), s.rows.iter().map(|e| e.path.clone()).collect::<Vec<_>>())
        };
        if picked.is_empty() {
            return;
        }
        std::thread::spawn(move || {
            let reach = roll_membership::reset_reach(&picked, &library);
            with_shell(move |c| c.confirm_reset_to_default(reach));
        });
    }

    fn confirm_reset_to_default(self: &Rc<Self>, reach: ResetReach) {
        let count = reach.frames.len();
        if count == 0 {
            return;
        }
        let sheets = !reach.sheets.is_empty();
        let title = trn("Reset This Photo to Default?", "Reset {n} Photos to Default?", count as u64);
        let message = if sheets {
            tr("Their roll's measurement and every setting of their look go, a frame shot in RGB passes splits back into its three shots, and the roll's contact sheet goes to the ONI Trash. Crops, dust corrections, flat-fields, medals and EXIF stay. Undo brings both back.")
        } else {
            tr("Their roll's measurement and every setting of their look go, a frame shot in RGB passes splits back into its three shots. Crops, dust corrections, flat-fields, medals and EXIF stay. Undo brings the settings back.")
        };
        let key = trn("Reset Photo", "Reset {n} Photos", count as u64);
        self.ask(&title, &message, &key, true, Pending::ResetToDefault(reach));
    }

    pub(super) fn reset_to_default(self: &Rc<Self>, reach: ResetReach) {
        let touched: Vec<String> = reach.frames.iter().chain(reach.sheets.iter()).cloned().collect();
        self.gesture(&touched, |store| store.reset_to_default(&reach));
    }

    // MARK: - Auto White Balance over several frames

    /// `press(.balance)`: the open frame alone balances at once; a selection asks Each or Average;
    /// frames shown together take the average of their balances, as one look across them.
    pub(super) fn auto_balance(self: &Rc<Self>) {
        if self.together_shown() {
            return self.run_auto_balance(Strategy::Average);
        }
        let count = self.state.borrow().balance_targets().len();
        if count == 0 || self.state.borrow().batch.is_some() {
            return;
        }
        if !choice::asks_choice(count) {
            return self.run_auto_balance(Strategy::Each);
        }
        let Some(w) = self.window() else { return };
        let wording = choice::wording(count);
        let c = w.global::<Choice>();
        c.set_title(wording.title.into());
        c.set_message(wording.detail.into());
        let keys = wording.options.iter().map(|(_, t)| crate::AskKey { text: t.as_str().into(), role: crate::AskRole::Option });
        c.set_keys(ModelRc::new(VecModel::from(keys.collect::<Vec<_>>())));
        c.set_shown(true);
        w.invoke_refocus();
        self.publish_bar();
    }

    fn balance_chosen(self: &Rc<Self>, i: Option<usize>) {
        if let Some(w) = self.window() {
            w.global::<Choice>().set_shown(false);
            w.invoke_refocus();
        }
        match i.and_then(|i| choice::STRATEGIES.get(i)) {
            Some(&strategy) => self.run_auto_balance(strategy),
            None => self.publish_bar(),
        }
    }

    /// `runAutoBalance`: each target read off this thread, the open frame on its live settings and
    /// the others on their own sidecars, the key showing how far the run is; one undo step.
    pub(super) fn run_auto_balance(self: &Rc<Self>, strategy: Strategy) {
        if self.state.borrow().batch.is_some() {
            return;
        }
        let frames: Vec<(String, PipelineSettings)> = {
            let s = self.state.borrow();
            let open = s.open_path().map(str::to_string);
            self.together_paths()
                .unwrap_or_else(|| s.balance_targets())
                .into_iter()
                .map(|path| {
                    let settings = if Some(&path) == open.as_ref() {
                        s.store.settings().clone()
                    } else {
                        let mosaic = oni_decode::decodes_via_mosaic(Path::new(&path));
                        oni_settings::sidecar::read(Path::new(&path), mosaic, true).map(|l| l.settings).unwrap_or_default()
                    };
                    (path, settings)
                })
                .collect()
        };
        if frames.is_empty() {
            return;
        }
        self.disarm_window();
        self.state.borrow_mut().batch = Some(Batch { done: 0, total: frames.len() });
        self.publish_batch();
        std::thread::spawn(move || {
            // Each frame's measuring copy is a decode, 0.35 s of the 0.4 s a frame takes, made off the
            // GPU's lock: a few at once, as Edit Together opens its frames.
            let frames = Arc::new(frames);
            let (next, done, outcomes) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)), Arc::new(Mutex::new(HashMap::new())));
            let workers: Vec<_> = (0..BALANCERS.min(frames.len()))
                .map(|_| {
                    let (frames, next, done, outcomes) = (frames.clone(), next.clone(), done.clone(), outcomes.clone());
                    std::thread::spawn(move || {
                        crate::decode_gate::in_background();
                        while let Some(frame) = frames.get(next.fetch_add(1, Ordering::Relaxed)) {
                            let found = crate::measuring::balance_outcomes(std::slice::from_ref(frame));
                            outcomes.lock().expect("outcomes").extend(found);
                            let count = done.fetch_add(1, Ordering::Relaxed) + 1;
                            with_shell(move |c| {
                                if let Some(b) = c.state.borrow_mut().batch.as_mut() {
                                    b.done = b.done.max(count);
                                }
                                c.publish_batch();
                            });
                        }
                    })
                })
                .collect();
            for worker in workers {
                let _ = worker.join();
            }
            let outcomes = std::mem::take(&mut *outcomes.lock().expect("outcomes"));
            let frames = Arc::try_unwrap(frames).unwrap_or_else(|f| (*f).clone());
            with_shell(move |c| {
                let targets: Vec<String> = frames.into_iter().map(|(p, _)| p).collect();
                let measure = |path: &str, _: &PipelineSettings| outcomes.get(path).cloned();
                let failure = {
                    let mut s = c.state.borrow_mut();
                    s.batch = None;
                    let failure = s.store.editor().and_then(|e| e.auto_balance(&targets, strategy, &measure));
                    s.last_change = Some(std::time::Instant::now());
                    failure
                };
                c.publish_batch();
                c.landed(&targets);
                if let Some(f) = failure {
                    c.say(&f);
                }
            });
        });
    }

    fn publish_batch(&self) {
        let Some(w) = self.window() else { return };
        let batch = self.state.borrow().batch;
        let panel = w.global::<PanelState>();
        panel.set_balancing(batch.is_some());
        if let Some(b) = batch {
            panel.set_balance_fraction(b.done as f32 / b.total.max(1) as f32);
            panel.set_balance_progress(choice::progress(b.done, b.total).into());
        }
    }
}

impl super::State {
    /// The balance key's targets, in the gallery's order: the selection, else the open frame.
    pub(super) fn balance_targets(&self) -> Vec<String> {
        crate::export::targets(&self.order, &self.selection(), self.open_path())
    }
}
