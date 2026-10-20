//! The transition from the earlier version: when a folder opens, the library's RAWs are named off this
//! thread; any camera the table lacks is matched to Apple's decode behind a blocking sheet. The
//! thumbnails' pass waits meanwhile, so no frame renders without its camera's matrix.

use super::{Ctl, with_shell};
use crate::{MainWindow, Transition as Sheet, TransitionLine};
use oni_transition::{AppleDecode, Event, Summary};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::Ordering;

#[derive(Default)]
pub(crate) struct Transition {
    /// Apple's decode; none in the checks and wherever the earlier app never ran.
    apple: Option<Arc<dyn AppleDecode>>,
    /// A search or a run under way, holding the thumbnails' pass.
    busy: bool,
    /// Every root was searched this session; a later folder is searched alone.
    searched: bool,
    /// Folders opened while a search or a run was under way, searched once it ends.
    waiting: Vec<String>,
    /// Each camera's name, its frame count and the line its match shows on.
    cameras: Vec<(String, usize, String)>,
    /// Cameras matched over all, for the progress bar.
    measured: (usize, usize),
}

impl Transition {
    /// The table on file goes into every decode before the first frame is read.
    pub(crate) fn new(apple: Option<Arc<dyn AppleDecode>>, support: &Path) -> Transition {
        if apple.is_some()
            && let Err(e) = oni_transition::table::install_from(support)
        {
            crate::timing::line(&format!("the camera table is not used: {e}"));
        }
        Transition { apple, ..Transition::default() }
    }
}

/// A camera's match as the sheet prints it: the density error left, as a share of the light.
pub fn residual_line(residual: &[f64; 3]) -> String {
    let worst = residual.iter().cloned().fold(0.0, f64::max);
    format!("matched, within {:.1} %", (10f64.powf(worst) - 1.0) * 100.0)
}

fn photos(n: usize) -> String {
    if n == 1 { "1 photo".into() } else { format!("{n} photos") }
}

impl Ctl {
    pub(super) fn wire_transition(me: &Rc<Ctl>, window: &MainWindow) {
        let p = me.clone();
        window.global::<Sheet>().on_continue_pressed(move || {
            if let Some(w) = p.window() {
                w.global::<Sheet>().set_shown(false);
            }
        });
    }

    /// Names the library's RAWs off this thread, every root the first time, then each folder opened.
    pub(super) fn transition_check(self: &Rc<Self>, folder: &str) {
        let (apple, support, roots) = {
            let mut s = self.state.borrow_mut();
            let Some(apple) = s.transition.apple.clone() else { return };
            s.transition.waiting.push(folder.to_string());
            if s.transition.busy {
                return;
            }
            s.transition.busy = true;
            s.hands.fetch_add(1, Ordering::Relaxed);
            let mut roots = if s.transition.searched { Vec::new() } else { oni_library::roots::Roots::load(&s.support).paths };
            roots.append(&mut s.transition.waiting);
            (apple, s.support.clone(), roots)
        };
        std::thread::Builder::new()
            .name("transition".into())
            .spawn(move || {
                // A panic in the search must still release the thumbnails' pass.
                let pending = std::panic::catch_unwind(|| oni_transition::pending(&support, &roots)).unwrap_or(0);
                with_shell(move |c| {
                    if pending > 0 {
                        c.transition_run(apple, support, roots);
                    } else {
                        c.transition_idle();
                    }
                });
            })
            .expect("the transition's thread");
    }

    /// The search or the run over: the thumbnails' pass goes on, and a folder opened meanwhile is searched.
    fn transition_idle(self: &Rc<Self>) {
        let next = {
            let mut s = self.state.borrow_mut();
            s.transition.busy = false;
            s.transition.searched = true;
            let _ = s.hands.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1));
            s.transition.waiting.pop()
        };
        if let Some(folder) = next {
            self.transition_check(&folder);
        }
    }

    /// The sheet over the window, then the run off this thread.
    fn transition_run(self: &Rc<Self>, apple: Arc<dyn AppleDecode>, support: PathBuf, roots: Vec<String>) {
        {
            let mut s = self.state.borrow_mut();
            s.transition.cameras.clear();
            s.transition.measured = (0, 0);
        }
        if let Some(w) = self.window() {
            let t = w.global::<Sheet>();
            t.set_done(false);
            t.set_progress(-1.0);
            t.set_stage("Looking for the cameras of your photos…".into());
            t.set_cameras(ModelRc::default());
            t.set_left(ModelRc::default());
            t.set_summary("".into());
            t.set_shown(true);
        }
        std::thread::Builder::new()
            .name("transition".into())
            .spawn(move || {
                let mut heard = |e: Event| with_shell(move |c| c.transition_heard(e));
                let result = oni_transition::run(&support, &roots, &*apple, &mut heard).map_err(|e| e.to_string());
                with_shell(move |c| c.transition_finished(result));
            })
            .expect("the transition's thread");
    }

    fn publish_cameras(&self) {
        let Some(w) = self.window() else { return };
        let lines: Vec<TransitionLine> = self
            .state
            .borrow()
            .transition
            .cameras
            .iter()
            .map(|(name, n, detail)| TransitionLine { camera: format!("{name} · {}", photos(*n)).into(), detail: detail.clone().into() })
            .collect();
        w.global::<Sheet>().set_cameras(ModelRc::new(VecModel::from(lines)));
    }

    fn transition_heard(&self, e: Event) {
        let Some(w) = self.window() else { return };
        let t = w.global::<Sheet>();
        match e {
            Event::Found { cameras } => {
                let mut s = self.state.borrow_mut();
                s.transition.measured = (0, cameras.len());
                s.transition.cameras = cameras.into_iter().map(|(name, n)| (name, n, "waiting".into())).collect();
                drop(s);
                self.publish_cameras();
            }
            Event::Measuring { camera } => {
                t.set_stage(format!("Matching the {camera} to the earlier version…").into());
                self.set_camera_detail(&camera, "matching…".into());
            }
            Event::Measured { camera, residual } => {
                let (done, of) = {
                    let mut s = self.state.borrow_mut();
                    s.transition.measured.0 += 1;
                    s.transition.measured
                };
                t.set_progress(done as f32 / of.max(1) as f32);
                self.set_camera_detail(&camera, residual.as_ref().map_or_else(|_| "not matched".into(), residual_line));
            }
        }
    }

    fn set_camera_detail(&self, camera: &str, detail: String) {
        if let Some(line) = self.state.borrow_mut().transition.cameras.iter_mut().find(|(n, _, _)| n == camera) {
            line.2 = detail;
        }
        self.publish_cameras();
    }

    fn transition_finished(self: &Rc<Self>, result: Result<Summary, String>) {
        self.transition_summary(result);
        self.transition_idle();
    }

    /// The summary and CONTINUE; the matched cameras' frames decoded and drawn again.
    fn transition_summary(self: &Rc<Self>, result: Result<Summary, String>) {
        let Some(w) = self.window() else { return };
        let t = w.global::<Sheet>();
        t.set_done(true);
        t.set_progress(1.0);
        let summary = match result {
            Ok(summary) => summary,
            Err(e) => {
                t.set_stage("Stopped".into());
                t.set_summary(format!("The cameras could not be matched: {e}. RAW files show LibRaw's own colours until the next launch.").into());
                return;
            }
        };
        t.set_stage("Done".into());
        let matched = summary.cameras.iter().filter(|c| c.residual.is_ok()).count();
        let mut text = match matched {
            0 => "No camera could be matched.".to_string(),
            1 => "1 camera matched.".to_string(),
            n => format!("{n} cameras matched."),
        };
        if !summary.unreachable.is_empty() {
            text += " Some folders could not be reached: their cameras are matched when they come back.";
        }
        t.set_summary(text.into());
        let left: Vec<slint::SharedString> =
            summary.cameras.iter().filter_map(|c| c.residual.as_ref().err().map(|why| format!("{}: {why}", c.name).into())).collect();
        t.set_left(ModelRc::new(VecModel::from(left)));
        self.transition_redraw(&summary.matched);
    }

    /// The frames of the cameras just matched: previews and thumbnails dropped, the open one read again.
    fn transition_redraw(self: &Rc<Self>, matched: &[PathBuf]) {
        let matched: HashSet<String> = matched.iter().map(|p| oni_library::paths::canonical(p)).collect();
        let (open, shown, support, previews) = {
            let mut s = self.state.borrow_mut();
            if s.full.as_ref().is_some_and(|(p, _)| matched.contains(p)) {
                s.full = None;
            }
            let shown: Vec<String> = matched.iter().filter(|p| s.fingerprints.contains_key(*p)).cloned().collect();
            (s.open.clone().filter(|p| matched.contains(p)), shown, s.support.clone(), s.previews.clone())
        };
        if let Ok(mut p) = previews.lock() {
            matched.iter().for_each(|path| p.remove(path));
        }
        let index = self.state.borrow().index_path.clone();
        if let Some(index) = oni_library::index::LibraryIndex::open(&index) {
            for e in index.all().into_iter().filter(|e| matched.contains(&e.path)) {
                let stale_thumbnail = crate::thumbnails::file(&support, &e.fingerprint);
                let _ = std::fs::remove_file(stale_thumbnail);
            }
        }
        if let Some(open) = open {
            self.open_photo(&open);
        }
        for p in &shown {
            self.redraw(p);
        }
    }
}
