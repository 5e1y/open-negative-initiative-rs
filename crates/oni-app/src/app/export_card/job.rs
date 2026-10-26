//! The export under way once the surface has closed: its destination shown in the file manager, its
//! modal over the canvas, its stop between frames, and how it ended.

use super::*;
use oni_i18n::{tr, trn};

/// How an export ended, which its modal says.
#[derive(Clone, Debug, PartialEq)]
pub enum End {
    Written,
    Stopped,
    /// The frame that failed and why, as the notice said it.
    Failed(String),
}

/// The export the modal follows, kept until the modal is closed.
pub struct Job {
    /// Each file's name, in the order written.
    pub names: Vec<String>,
    pub stopping: bool,
    pub end: Option<End>,
}

/// Where the destination folder is shown: the file manager, or a check's own record.
pub type Reveal = Rc<dyn Fn(&Path)>;

impl Ctl {
    /// The surface closes on the export's start, back where it was opened from, and the destination
    /// opens in the Finder or the Explorer while the modal counts the frames.
    pub(super) fn job_started(self: &Rc<Self>, destinations: &[PathBuf]) {
        let folder = destinations.first().and_then(|d| d.parent()).map(Path::to_path_buf).unwrap_or_default();
        let names = destinations.iter().map(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()).collect();
        self.export.borrow_mut().job = Some(Job { names, stopping: false, end: None });
        self.leave_export();
        self.publish_export();
        let reveal = {
            let c = self.export.borrow();
            // A check answering the panels opens no file manager unless it hands its own record.
            c.reveal.clone().or_else(|| c.chooser.is_none().then(|| Rc::new(crate::system::show_folder) as Reveal))
        };
        if let Some(reveal) = reveal {
            reveal(&folder);
        }
    }

    /// The batch ended, stopped or failed: the modal says so, the surface's key is free again.
    pub(super) fn export_ended(self: &Rc<Self>, end: End) {
        {
            let mut c = self.export.borrow_mut();
            c.exporting = false;
            if end == End::Stopped {
                c.stopped = Some(c.progress);
            }
            if let Some(job) = c.job.as_mut() {
                job.end = Some(end);
            }
        }
        self.publish_export();
    }

    /// The modal's Stop: the frame under way is finished, none after it begun.
    pub(super) fn stop_job(&self) {
        {
            let mut c = self.export.borrow_mut();
            if !c.exporting {
                return;
            }
            c.stop.store(true, Ordering::Relaxed);
            if let Some(job) = c.job.as_mut() {
                job.stopping = true;
            }
        }
        self.publish_job();
    }

    /// The modal closed by its key, or once an export that went well has lingered.
    pub(super) fn close_job(&self) {
        {
            let mut c = self.export.borrow_mut();
            if c.exporting {
                return;
            }
            c.job = None;
        }
        self.publish_job();
    }

    /// The engine an export renders on: one apart, made once, so the editor's renders never wait a frame.
    pub(super) fn export_engine(&self) -> Shared {
        let kept = self.export.borrow().apart.clone();
        if let Some(apart) = kept {
            return apart;
        }
        match self.engine.apart() {
            Ok(apart) => {
                self.export.borrow_mut().apart = Some(apart.clone());
                apart
            }
            Err(_) => self.engine.clone(),
        }
    }

    pub(super) fn publish_job(&self) {
        let Some(w) = self.window() else { return };
        let c = self.export.borrow();
        let e = w.global::<Exporting>();
        e.set_job_shown(c.job.is_some());
        let Some(job) = &c.job else { return };
        let total = job.names.len();
        let done = c.progress.min(total);
        let running = job.end.is_none();
        e.set_job_running(running);
        e.set_job_stopping(job.stopping);
        e.set_job_failed(matches!(job.end, Some(End::Failed(_))));
        e.set_job_fraction(if total > 0 { done as f32 / total as f32 } else { 0.0 });
        e.set_job_count(tr!("{done} of {total}", done = done, total = total).into());
        let current = if running { job.names.get(done).cloned().unwrap_or_default() } else { String::new() };
        e.set_job_name(current.into());
        let title = match &job.end {
            None if job.stopping => tr("Stopping after this photo…").to_owned(),
            None => tr("Exporting…").to_owned(),
            Some(End::Written) => trn("{n} photo exported", "{n} photos exported", done as u64),
            Some(End::Stopped) => trn("Stopped after {n} file was written.", "Stopped after {n} files were written.", done as u64),
            Some(End::Failed(_)) => tr("Export Failed").to_owned(),
        };
        e.set_job_title(title.into());
        let message = match &job.end {
            Some(End::Failed(m)) => m.clone(),
            _ => String::new(),
        };
        e.set_job_message(message.into());
    }
}

/// The export the modal follows, read back by the checks.
pub fn job(ctl: &Ctl) -> Option<(usize, usize, Option<End>)> {
    let c = ctl.export.borrow();
    c.job.as_ref().map(|j| (c.progress.min(j.names.len()), j.names.len(), j.end.clone()))
}

/// A check records the folder an export shows, never opening the file manager.
pub fn reveal_with(ctl: &Ctl, reveal: Reveal) {
    ctl.export.borrow_mut().reveal = Some(reveal);
}
