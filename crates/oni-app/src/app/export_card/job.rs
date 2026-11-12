//! The export under way once the surface has closed: its progress and Stop in the status bar, its
//! stop between frames, and how it ended, told in the notice whose key shows the files written.

use super::*;
use oni_i18n::{tr, trn};

/// How an export ended, which its notice says.
#[derive(Clone, Debug, PartialEq)]
pub enum End {
    Written,
    Stopped,
    /// The frame that failed and why, its name marked for the log.
    Failed(String),
}

/// The export the status bar follows, kept once ended for the next one to replace.
pub struct Job {
    /// Each file's name, in the order written.
    pub names: Vec<String>,
    /// The first file, which the notice's Show in Finder selects in its folder.
    first: PathBuf,
    pub stopping: bool,
    pub end: Option<End>,
}

impl Ctl {
    /// The surface closes on the export's start, back where it was opened from, while the status
    /// bar counts the frames.
    pub(super) fn job_started(self: &Rc<Self>, destinations: &[PathBuf]) {
        let names = destinations.iter().map(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()).collect();
        let first = destinations.first().cloned().unwrap_or_default();
        self.export.borrow_mut().job = Some(Job { names, first, stopping: false, end: None });
        self.leave_export();
        self.publish_export();
    }

    /// The batch ended, stopped or failed: the notice says so, with Show in Finder once a file is
    /// written, and the surface's key is free again.
    pub(super) fn export_ended(self: &Rc<Self>, end: End) {
        let (done, first) = {
            let mut c = self.export.borrow_mut();
            c.exporting = false;
            if end == End::Stopped {
                c.stopped = Some(c.progress);
            }
            let done = c.progress;
            let Some(job) = c.job.as_mut() else { return };
            job.end = Some(end.clone());
            (done.min(job.names.len()), job.first.clone())
        };
        self.publish_export();
        let (title, message) = match &end {
            End::Written => (tr("Export"), trn("{n} photo exported", "{n} photos exported", done as u64)),
            End::Stopped => (tr("Export"), trn("Stopped after {n} file was written.", "Stopped after {n} files were written.", done as u64)),
            End::Failed(why) => (tr("Export Failed"), why.clone()),
        };
        self.told_with(title, &message, (done > 0).then(|| first.to_string_lossy().into_owned()));
    }

    /// The status bar's Stop: the frame under way is finished, none after it begun.
    pub(super) fn stop_job(&self) {
        {
            let mut c = self.export.borrow_mut();
            if !c.exporting {
                return;
            }
            c.writer.halt();
            if let Some(job) = c.job.as_mut() {
                job.stopping = true;
            }
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
        let running = c.job.as_ref().filter(|j| j.end.is_none());
        e.set_job_shown(running.is_some());
        let Some(job) = running else { return };
        let total = job.names.len();
        let done = c.progress.min(total);
        e.set_job_stopping(job.stopping);
        e.set_job_fraction(if total > 0 { done as f32 / total as f32 } else { 0.0 });
        e.set_job_count(tr!("{done} of {total}", done = done, total = total).into());
        e.set_job_title(if job.stopping { tr("Stopping after this photo…") } else { tr("Exporting…") }.into());
    }
}

/// The export the status bar follows, read back by the checks.
pub fn job(ctl: &Ctl) -> Option<(usize, usize, Option<End>)> {
    let c = ctl.export.borrow();
    c.job.as_ref().map(|j| (c.progress.min(j.names.len()), j.names.len(), j.end.clone()))
}
