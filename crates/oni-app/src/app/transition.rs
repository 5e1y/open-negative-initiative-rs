//! The transition from the earlier version: when a folder opens, the library is searched off this
//! thread for RAW sidecars the Swift app wrote; any found are corrected behind a blocking sheet.

use super::{Ctl, with_shell};
use crate::{MainWindow, Transition as Sheet, TransitionLine};
use oni_transition::{AppleDecode, Event, Summary};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

/// The lines naming frames left as they were, the rest counted.
const LEFT_SHOWN: usize = 6;

#[derive(Default)]
pub(crate) struct Transition {
    /// Apple's decode; none in the checks and wherever the earlier app never ran.
    apple: Option<Arc<dyn AppleDecode>>,
    /// A search or a run under way: a folder opened meanwhile asks nothing more.
    busy: bool,
    /// Every root was searched this session; a later folder adds itself and the disks unplugged before.
    searched: bool,
    /// Folders opened while a search or a run was under way, searched once it ends.
    waiting: Vec<String>,
    /// Each camera's name, its frame count and the line its shift shows on.
    cameras: Vec<(String, usize, String)>,
}

impl Transition {
    pub(crate) fn new(apple: Option<Arc<dyn AppleDecode>>) -> Transition {
        Transition { apple, ..Transition::default() }
    }
}

/// A camera's shift as the sheet prints it, in stops.
pub fn shift_line(stops: &[f64; 3]) -> String {
    format!("R {:+.3}  G {:+.3}  B {:+.3} stop", stops[0], stops[1], stops[2]).replace('-', "\u{2212}")
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

    /// Searches the library for the earlier version's sidecars off this thread: every root the
    /// first time, then each folder opened and the roots unreachable before.
    pub(super) fn transition_check(self: &Rc<Self>, folder: &str) {
        let (apple, support, roots) = {
            let mut s = self.state.borrow_mut();
            let Some(apple) = s.transition.apple.clone() else { return };
            s.transition.waiting.push(folder.to_string());
            if s.transition.busy {
                return;
            }
            s.transition.busy = true;
            let mut roots = if s.transition.searched { Vec::new() } else { oni_library::roots::Roots::load(&s.support).paths };
            roots.append(&mut s.transition.waiting);
            (apple, s.support.clone(), roots)
        };
        std::thread::Builder::new()
            .name("transition".into())
            .spawn(move || {
                let pending = oni_transition::pending(&support, &roots);
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

    /// The search or the run over; a folder opened meanwhile is searched now.
    fn transition_idle(self: &Rc<Self>) {
        let next = {
            let mut s = self.state.borrow_mut();
            s.transition.busy = false;
            s.transition.searched = true;
            s.transition.waiting.pop()
        };
        if let Some(folder) = next {
            self.transition_check(&folder);
        }
    }

    /// The sheet over the window, the open frame's settings written first, then the run off this thread.
    fn transition_run(self: &Rc<Self>, apple: Arc<dyn AppleDecode>, support: PathBuf, roots: Vec<String>) {
        if let Some(failure) = self.last_writes() {
            self.say(&format!("The open frame's settings could not be saved: {failure}"));
        }
        self.state.borrow_mut().transition.cameras.clear();
        if let Some(w) = self.window() {
            let t = w.global::<Sheet>();
            t.set_done(false);
            t.set_progress(-1.0);
            t.set_stage("Looking for photos prepared in the earlier version…".into());
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
                self.state.borrow_mut().transition.cameras = cameras.into_iter().map(|(name, n)| (name, n, "waiting".into())).collect();
                self.publish_cameras();
            }
            Event::Measuring { camera } => {
                t.set_progress(-1.0);
                t.set_stage(format!("Measuring the {camera} against the earlier decoder…").into());
                self.set_camera_detail(&camera, "measuring…".into());
            }
            Event::Measured { camera, stops } => {
                self.set_camera_detail(&camera, stops.as_ref().map_or_else(|_| "not measured".into(), shift_line));
            }
            Event::Corrected { done, of } => {
                t.set_progress(done as f32 / of.max(1) as f32);
                t.set_stage(format!("Correcting {done} of {}", photos(of)).into());
            }
        }
    }

    fn set_camera_detail(&self, camera: &str, detail: String) {
        if let Some(line) = self.state.borrow_mut().transition.cameras.iter_mut().find(|(n, _, _)| n == camera) {
            line.2 = detail;
        }
        self.publish_cameras();
    }

    /// The summary and CONTINUE; the open frame read again and the corrected photos' thumbnails redrawn.
    fn transition_finished(self: &Rc<Self>, result: Result<Summary, String>) {
        self.transition_summary(result);
        self.transition_idle();
    }

    fn transition_summary(self: &Rc<Self>, result: Result<Summary, String>) {
        let Some(w) = self.window() else { return };
        let t = w.global::<Sheet>();
        t.set_done(true);
        t.set_progress(1.0);
        let summary = match result {
            Ok(summary) => summary,
            Err(e) => {
                t.set_stage("Stopped".into());
                t.set_summary(format!("The correction stopped: {e}. Nothing is lost; it starts again at the next launch.").into());
                return;
            }
        };
        t.set_stage("Done".into());
        let mut text = match summary.corrected.len() {
            0 => "No photo needed a correction.".to_string(),
            n => format!("{} corrected.", photos(n)),
        };
        if !summary.left.is_empty() {
            text += &format!(" {} left as they were:", photos(summary.left.len()));
        }
        if !summary.unreachable.is_empty() {
            text += " Some folders could not be reached: their photos are corrected when they come back.";
        }
        t.set_summary(text.into());
        let mut left: Vec<slint::SharedString> = summary
            .left
            .iter()
            .take(LEFT_SHOWN)
            .map(|(raw, why)| format!("{}: {why}", raw.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned())).into())
            .collect();
        if summary.left.len() > LEFT_SHOWN {
            left.push(format!("and {} more", summary.left.len() - LEFT_SHOWN).into());
        }
        t.set_left(ModelRc::new(VecModel::from(left)));
        self.transition_reload(&summary);
    }

    /// The open frame read again from its corrected sidecar, and each corrected photo's thumbnail redrawn.
    fn transition_reload(self: &Rc<Self>, summary: &Summary) {
        let corrected: HashSet<String> = summary.corrected.iter().map(|p| p.to_string_lossy().into_owned()).collect();
        let (open, shown, support) = {
            let s = self.state.borrow();
            let shown: Vec<String> = corrected.iter().filter(|p| s.fingerprints.contains_key(*p)).cloned().collect();
            (s.open.clone().filter(|p| corrected.contains(p)), shown, s.support.clone())
        };
        if let Some(open) = open {
            self.open_photo(&open);
        }
        for p in &shown {
            self.redraw(p);
        }
        // The thumbnails of photos outside the open folder are dropped, so their pass draws them again.
        let index = self.state.borrow().index_path.clone();
        if let Some(index) = oni_library::index::LibraryIndex::open(&index) {
            for e in index.all().into_iter().filter(|e| corrected.contains(&e.path) && !shown.contains(&e.path)) {
                let stale_thumbnail = crate::thumbnails::file(&support, &e.fingerprint);
                let _ = std::fs::remove_file(stale_thumbnail);
            }
        }
    }
}
