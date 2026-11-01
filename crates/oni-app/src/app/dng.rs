//! A folder's Nikon High Efficiency RAW made DNGs on a click: Adobe DNG Converter fetched and
//! installed when missing, the frames converted beside their NEF files, then the NEFs kept or trashed
//! as the person chooses, and the editor or the roll going on with the DNGs.

use super::{Ctl, with_shell};
use crate::adobe_dng;
use crate::{Converting, DngStage};
use oni_i18n::{tr, trn};
use slint::ComponentHandle;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// How often a running job's count reaches the sheet.
const TICK: Duration = Duration::from_millis(100);
/// How long the folder read again is awaited before the frames converted are opened.
const SETTLED: Duration = Duration::from_secs(20);

/// Where the click came from, which the DNGs then take over.
#[derive(Clone)]
enum Origin {
    Editor(String),
    Roll(Vec<String>),
}

struct Job {
    folder: PathBuf,
    origin: Origin,
    stop: Arc<AtomicBool>,
    converted: Vec<PathBuf>,
}

thread_local! {
    static JOB: RefCell<Option<Job>> = const { RefCell::new(None) };
    /// Polls the folder read again until the DNGs taken up are listed.
    static WAIT: slint::Timer = slint::Timer::default();
}

fn megabytes(bytes: u64) -> u64 {
    bytes.div_ceil(1_000_000)
}

impl Ctl {
    pub(in crate::app) fn wire_dng(self: &Rc<Self>) {
        let Some(w) = self.window() else { return };
        let c = w.global::<Converting>();
        let me = self.clone();
        c.on_offered(move || me.dng_offered());
        let me = self.clone();
        c.on_install(move || me.dng_install());
        let me = self.clone();
        c.on_cancelled(move || me.dng_closed());
        let me = self.clone();
        c.on_closed(move || me.dng_closed());
        let me = self.clone();
        c.on_kept(move |dngs_only| me.dng_kept(dngs_only));
    }

    fn dng_sheet(&self, stage: DngStage, message: &str, fraction: f32, count: &str) {
        let Some(w) = self.window() else { return };
        let c = w.global::<Converting>();
        c.set_stage(stage);
        c.set_message(message.into());
        c.set_fraction(fraction);
        c.set_count(count.into());
        c.set_shown(true);
    }

    fn dng_failed(&self, why: &str) {
        JOB.with(|j| *j.borrow_mut() = None);
        self.dng_sheet(DngStage::Failed, why, -1.0, "");
    }

    fn dng_running(stop: &Arc<AtomicBool>) -> bool {
        JOB.with(|j| j.borrow().as_ref().is_some_and(|job| Arc::ptr_eq(&job.stop, stop)))
    }

    /// The failed frame's folder, from the roll's base step or the editor; its converter next.
    pub(in crate::app) fn dng_offered(self: &Rc<Self>) {
        let (origin, path) = {
            let s = self.state.borrow();
            if s.prepare.shown {
                let Some(base) = s.prepare.base_path() else { return };
                (Origin::Roll(s.prepare.frame_paths()), base)
            } else {
                let Some(open) = s.open_path() else { return };
                (Origin::Editor(open.to_string()), open.to_string())
            }
        };
        let Some(folder) = Path::new(&path).parent().map(Path::to_path_buf) else { return };
        let stop = Arc::new(AtomicBool::new(false));
        JOB.with(|j| *j.borrow_mut() = Some(Job { folder, origin, stop, converted: Vec::new() }));
        match adobe_dng::installed() {
            Some(program) => self.dng_convert(program),
            None => self.dng_sheet(
                DngStage::Offer,
                &tr("ONI reads Nikon’s High Efficiency RAW through Adobe DNG Converter, free from Adobe, which writes a lossless DNG beside each file. ONI downloads it from Adobe (about 1.8 GB), then opens its installer."),
                -1.0,
                "",
            ),
        }
    }

    /// The pinned installer fetched off this thread, then opened; the conversion follows its close.
    fn dng_install(self: &Rc<Self>) {
        let Some(stop) = JOB.with(|j| j.borrow().as_ref().map(|job| job.stop.clone())) else { return };
        self.dng_sheet(DngStage::Fetching, &tr("Downloading Adobe DNG Converter from Adobe…"), 0.0, "");
        let folder = crate::system::caches().join("Adobe DNG Converter");
        std::thread::spawn(move || {
            let told = std::sync::Mutex::new(Instant::now());
            let going = |done: u64, total: Option<u64>| {
                let mut at = told.lock().unwrap_or_else(|e| e.into_inner());
                if at.elapsed() >= TICK {
                    *at = Instant::now();
                    let (stop, fraction) = (stop.clone(), total.map_or(-1.0, |t| done as f32 / t.max(1) as f32));
                    let count = match total {
                        Some(t) => tr!("{done} MB of {total} MB", done = megabytes(done), total = megabytes(t)),
                        None => tr!("{done} MB", done = megabytes(done)),
                    };
                    with_shell(move |c| {
                        if Ctl::dng_running(&stop) {
                            c.dng_sheet(DngStage::Fetching, &tr("Downloading Adobe DNG Converter from Adobe…"), fraction, &count);
                        }
                    });
                }
                !stop.load(Ordering::Relaxed)
            };
            let fetched = adobe_dng::fetch(&folder, &going);
            with_shell(move |c| c.dng_fetched(fetched));
        });
    }

    fn dng_fetched(self: &Rc<Self>, fetched: Result<PathBuf, String>) {
        let Some(stop) = JOB.with(|j| j.borrow().as_ref().map(|job| job.stop.clone())) else { return };
        let installer = match fetched {
            Ok(installer) => installer,
            Err(e) if e.is_empty() => return,
            Err(e) => return self.dng_failed(&tr!("Adobe DNG Converter could not be downloaded: {error}", error = e)),
        };
        self.dng_sheet(
            DngStage::Installing,
            &tr("Adobe’s installer is open: follow it to the end. ONI converts the photos once Adobe DNG Converter is installed."),
            -1.0,
            "",
        );
        std::thread::spawn(move || {
            let ran = adobe_dng::run_installer(&installer);
            if adobe_dng::installed().is_some() {
                let _ = std::fs::remove_file(&installer);
            }
            with_shell(move |c| {
                if !Ctl::dng_running(&stop) {
                    return;
                }
                match (adobe_dng::installed(), ran) {
                    (Some(program), _) => c.dng_convert(program),
                    (None, Err(e)) => c.dng_failed(&tr!("Adobe DNG Converter could not be installed: {error}", error = e)),
                    (None, Ok(())) => c.dng_failed(&tr("Adobe DNG Converter is not installed: its installer closed before the end.")),
                }
            });
        });
    }

    /// The folder's High Efficiency NEFs converted off this thread, the count moving on the sheet.
    fn dng_convert(self: &Rc<Self>, program: PathBuf) {
        let Some((folder, stop)) = JOB.with(|j| j.borrow().as_ref().map(|job| (job.folder.clone(), job.stop.clone()))) else { return };
        let converting = tr("Converting the folder’s High Efficiency NEF files to DNG…");
        self.dng_sheet(DngStage::Converting, &converting, -1.0, "");
        std::thread::spawn(move || {
            let raws = adobe_dng::convertible(&folder);
            let total = raws.len();
            let going = |done: usize| {
                let (held, converting) = (stop.clone(), converting.clone());
                with_shell(move |c| {
                    if Ctl::dng_running(&held) {
                        let count = tr!("{done} of {total}", done = done, total = total);
                        c.dng_sheet(DngStage::Converting, &converting, done as f32 / total.max(1) as f32, &count);
                    }
                });
                !stop.load(Ordering::Relaxed)
            };
            let converted = if raws.is_empty() { Ok(Vec::new()) } else { adobe_dng::convert(&program, &raws, &going) };
            with_shell(move |c| c.dng_converted(converted));
        });
    }

    fn dng_converted(self: &Rc<Self>, converted: Result<Vec<PathBuf>, String>) {
        let converted = match converted {
            Ok(converted) => converted,
            Err(e) => return self.dng_failed(&e),
        };
        let count = converted.len();
        let present = JOB.with(|j| {
            j.borrow_mut().as_mut().map(|job| job.converted = converted).is_some()
        });
        if !present {
            return;
        }
        if count == 0 {
            return self.dng_failed(&tr("The folder holds no High Efficiency NEF file left to convert."));
        }
        let message = trn("One DNG stands beside its NEF file, which ONI no longer shows.", "{n} DNGs stand beside their NEF files, which ONI no longer shows.", count as u64);
        self.dng_sheet(DngStage::Choice, &message, 1.0, "");
    }

    /// The NEFs sent to the Trash or kept, then the folder read again and the DNGs taken up.
    fn dng_kept(self: &Rc<Self>, dngs_only: bool) {
        let Some(job) = JOB.with(|j| j.borrow_mut().take()) else { return };
        if let Some(w) = self.window() {
            w.global::<Converting>().set_shown(false);
        }
        let mut refused = 0;
        if dngs_only {
            let s = self.state.borrow();
            for raw in &job.converted {
                if (s.discard)(raw).is_err() {
                    refused += 1;
                }
            }
        }
        if refused > 0 {
            self.say(&trn("One NEF file could not be moved to the Trash.", "{n} NEF files could not be moved to the Trash.", refused as u64));
        }
        self.dng_go_on(job);
    }

    fn dng_closed(self: &Rc<Self>) {
        if let Some(job) = JOB.with(|j| j.borrow_mut().take()) {
            job.stop.store(true, Ordering::Relaxed);
        }
        if let Some(w) = self.window() {
            w.global::<Converting>().set_shown(false);
        }
    }

    /// The editor opens the DNG of the frame it failed on; the roll comes back on its frames' DNGs.
    fn dng_go_on(self: &Rc<Self>, job: Job) {
        let dng = |path: &str| {
            let raw = Path::new(path);
            if job.converted.iter().any(|c| c == raw) { adobe_dng::twin(raw).to_string_lossy().into_owned() } else { path.to_string() }
        };
        let origin = match &job.origin {
            Origin::Editor(path) => Origin::Editor(dng(path)),
            Origin::Roll(paths) => Origin::Roll(paths.iter().map(|p| dng(p)).collect()),
        };
        if matches!(origin, Origin::Roll(_)) {
            self.leave_roll();
        }
        self.refresh();
        let started = Instant::now();
        let me = self.clone();
        WAIT.with(|w| {
            w.start(slint::TimerMode::Repeated, TICK, move || {
                let wanted = match &origin {
                    Origin::Editor(path) => std::slice::from_ref(path),
                    Origin::Roll(paths) => paths.as_slice(),
                };
                let listed = {
                    let s = me.state.borrow();
                    s.read_done() && wanted.iter().all(|p| s.rows.iter().any(|e| e.path == *p))
                };
                if !listed && started.elapsed() < SETTLED {
                    return;
                }
                WAIT.with(|w| w.stop());
                match &origin {
                    Origin::Editor(path) => me.open_photo(path),
                    Origin::Roll(paths) => me.enter_roll(paths, 0),
                }
            });
        });
    }
}
