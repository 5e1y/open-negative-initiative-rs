//! The export surface's state and gestures: entering and leaving, each target's line, the previews
//! and the estimate off this thread, the space the canvas is proofed in, and the write.

use super::{Ctl, image, prepared, with_shell};
use crate::display::Params;
use crate::export::{self, Target, label};
use crate::preferences::Preferences;
use crate::proof::{self, Wanted};
use crate::render::Shared;
use crate::track::{self, DOUBLE_CLICK};
use crate::{App, ExportLine, Exporting, MainWindow};
use oni_colour::{OutputProfile, Proof, Space};
use oni_export::{ExifMetadata, Format, PREVIEW_SIDE, Preset, Resize, Row, Settings};
use oni_library::journal::{Journal, Kind as Gesture};
use oni_settings::film::FILM_STOCKS;
use serde_json::Value;
use slint::{ComponentHandle, Image, Model, ModelRc, SharedString, VecModel};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

#[path = "paper_sheet.rs"]
pub mod paper_sheet;

pub const PROOF_COLOUR_ONLY: &str = "proofColourOnly";
pub const SCREEN_GAMUT: &str = "screenGamutWarning";
pub const ARTIST: &str = "exportArtist";
pub const COPYRIGHT: &str = "exportCopyright";
pub const LAST_PROFILE: &str = "exportLastProfile";

/// `DSSlider`'s range and rest for the JPEG quality.
const QUALITY: (f64, f64) = (0.3, 1.0);
const QUALITY_BASE: f64 = 0.85;

/// What a panel asks the hand; a check answers in its place, the system's panels otherwise.
#[derive(Clone, Debug, PartialEq)]
pub enum Ask {
    Save { name: String, extension: String },
    Folder { count: usize },
    Profiles,
}

pub type Chooser = Rc<dyn Fn(&Ask) -> Vec<PathBuf>>;

#[derive(Clone)]
enum Preview {
    Waiting,
    Ready(Image),
    Failed,
}

#[derive(Default)]
struct QualityHand {
    start: Option<f32>,
    last_press: Option<Instant>,
}

pub struct Card {
    pub shown: bool,
    return_gallery: bool,
    /// The space the frames were edited in when the surface opened, which the note compares to.
    edited_in: Option<Space>,
    pub settings: Settings,
    pub colour_only: bool,
    pub gamut: bool,
    pub targets: Vec<Target>,
    pub rows: Vec<Option<Row>>,
    sizes: Vec<Option<(f64, f64)>>,
    previews: Vec<Preview>,
    low_precision: usize,
    estimate: Option<u64>,
    pub exporting: bool,
    pub progress: usize,
    stopped: Option<usize>,
    stop: Arc<AtomicBool>,
    /// Each batch's pass and each estimate carry a generation; a newer one makes them stop.
    pass: Arc<AtomicU64>,
    weighing: Arc<AtomicU64>,
    artist: String,
    copyright: String,
    last_profile: String,
    pub installed: Vec<OutputProfile>,
    pending: bool,
    pub wanted: Wanted,
    prefs: Preferences,
    /// The folder a loaded profile is installed into.
    pub profiles: PathBuf,
    support: PathBuf,
    pub chooser: Option<Chooser>,
    confirm: Option<Vec<PathBuf>>,
    quality: QualityHand,
    film_row: Option<usize>,
    pub notice: Option<(String, String)>,
    model: Rc<VecModel<ExportLine>>,
    pub papers: paper_sheet::Sheet,
}

/// `AppFolders.colorSyncProfiles`: the user's own profile folder.
fn user_profiles(support: &Path) -> PathBuf {
    if cfg!(target_os = "macos")
        && let Some(home) = std::env::var_os("HOME")
    {
        return PathBuf::from(home).join("Library/ColorSync/Profiles");
    }
    support.join("Profiles")
}

impl Card {
    pub fn new(support: &Path) -> Card {
        let prefs = Preferences::load(support);
        let settings = prefs.string(oni_export::settings::PREFERENCE_KEY).and_then(Settings::from_raw_value).unwrap_or_default();
        let colour_only = prefs.bool(PROOF_COLOUR_ONLY).unwrap_or(false);
        let text = |k: &str| prefs.string(k).unwrap_or_default().to_owned();
        Card {
            shown: false,
            return_gallery: true,
            edited_in: None,
            wanted: proof::wanted(settings.space().clone(), colour_only),
            colour_only,
            gamut: prefs.bool(SCREEN_GAMUT).unwrap_or(false),
            artist: text(ARTIST),
            copyright: text(COPYRIGHT),
            last_profile: text(LAST_PROFILE),
            settings,
            targets: Vec::new(),
            rows: Vec::new(),
            sizes: Vec::new(),
            previews: Vec::new(),
            low_precision: 0,
            estimate: None,
            exporting: false,
            progress: 0,
            stopped: None,
            stop: Arc::default(),
            pass: Arc::default(),
            weighing: Arc::default(),
            installed: OutputProfile::last_installed(),
            pending: false,
            prefs,
            profiles: user_profiles(support),
            support: support.to_path_buf(),
            chooser: None,
            confirm: None,
            quality: QualityHand::default(),
            film_row: None,
            notice: None,
            model: Rc::new(VecModel::default()),
            papers: paper_sheet::Sheet::default(),
        }
    }

    /// The first target's full size after its turns and crop, once measured.
    fn full(&self) -> Option<(f64, f64)> {
        self.sizes.first().copied().flatten()
    }

    fn save(&mut self, key: &str, value: Value) {
        let _ = self.prefs.set(key, value);
    }

    fn save_settings(&mut self) {
        let raw = self.settings.raw_value();
        self.save(oni_export::settings::PREFERENCE_KEY, Value::from(raw));
    }

    fn line(&self, i: usize) -> ExportLine {
        let row = self.rows.get(i).cloned().flatten();
        let m = row.as_ref().map(|r| r.metadata.clone()).unwrap_or_default();
        let target = &self.targets[i];
        let (preview, state) = match self.previews.get(i) {
            Some(Preview::Ready(img)) => (img.clone(), 1),
            Some(Preview::Failed) => (Image::default(), 2),
            _ => (Image::default(), 0),
        };
        let size = self.sizes.get(i).copied().flatten();
        let caption = size.map(|(w, h)| {
            let (ow, oh) = self.settings.resize.output_size(w, h);
            format!("{} × {}", ow as i64, oh as i64)
        });
        ExportLine {
            name: row.as_ref().map_or_else(|| oni_export::stem(&target.path), |r| r.name.clone()).into(),
            camera: m.camera.into(),
            lens: m.lens.into(),
            film: m.film.into(),
            iso: m.iso.into(),
            date: m.date.into(),
            artist: m.artist.into(),
            copyright: m.copyright.into(),
            ready: row.is_some(),
            preview,
            state,
            caption: caption.unwrap_or_default().into(),
            aspect: size.map_or(0.0, |(w, h)| if h > 0.0 { (w / h) as f32 } else { 0.0 }),
        }
    }

    /// `ExportSpaceField.options`: titles kept distinct, a shared description gaining its file name.
    fn profile_titles(&self, chosen: &OutputProfile) -> (Vec<OutputProfile>, Vec<String>) {
        let listed = export::listed_profiles(&self.installed, chosen);
        let titles = export::profile_titles(&listed);
        (listed, titles)
    }
}


impl Ctl {
    // MARK: - Entering and leaving

    /// Menu, bar and ⌘E: on the surface they confirm, everywhere else they open it.
    pub(super) fn export_act(self: &Rc<Self>, action: &str) {
        match action {
            "export" if self.export.borrow().shown => self.run_export(),
            "export" => self.export_now(),
            _ => {}
        }
    }

    /// `screenGamutWarning` kept for the next launch, and the View menu's word for it.
    pub(super) fn gamut_kept(&self) {
        let on = self.state.borrow().screen_gamut;
        {
            let mut c = self.export.borrow_mut();
            c.gamut = on;
            c.save(SCREEN_GAMUT, Value::from(on));
        }
        self.publish_export();
    }

    /// `exportNow`: the selection in gallery order, else the open photo.
    pub fn export_now(self: &Rc<Self>) {
        let targets = {
            let s = self.state.borrow();
            let paths = export::targets(&s.order, &s.selection(), s.open.as_deref());
            let open = s.open_path().map(str::to_string);
            let current = s.store.settings().clone();
            paths
                .into_iter()
                .map(|p| {
                    let settings = if Some(&p) == open.as_ref() {
                        current.clone()
                    } else {
                        let mosaic = oni_decode::decodes_via_mosaic(Path::new(&p));
                        oni_settings::sidecar::read(Path::new(&p), mosaic, true).map(|l| l.settings).unwrap_or_default()
                    };
                    Target { path: PathBuf::from(p), settings }
                })
                .collect::<Vec<_>>()
        };
        if targets.is_empty() {
            return;
        }
        self.flush();
        self.enter_export(targets);
    }

    /// `enterExport`: entering from the surface keeps the earlier return, or Cancel would land on it.
    pub fn enter_export(self: &Rc<Self>, targets: Vec<Target>) {
        let gallery = self.window().is_some_and(|w| w.global::<App>().get_gallery());
        {
            let mut c = self.export.borrow_mut();
            if !c.shown {
                c.return_gallery = gallery;
                c.edited_in = Some(c.settings.space().clone());
            }
            c.shown = true;
            c.stopped = None;
            c.stop.store(false, Ordering::Relaxed);
            c.progress = 0;
            c.rows = vec![None; targets.len()];
            c.sizes = vec![None; targets.len()];
            c.previews = vec![Preview::Waiting; targets.len()];
            c.low_precision = 0;
            c.estimate = None;
            c.targets = targets;
        }
        self.publish_lines();
        self.publish_export();
        self.read_targets();
    }

    /// `leaveExport`: back where the surface was opened from.
    pub fn leave_export(self: &Rc<Self>) {
        let back = {
            let mut c = self.export.borrow_mut();
            if !c.shown {
                return;
            }
            c.shown = false;
            c.stopped = None;
            c.pass.fetch_add(1, Ordering::Relaxed);
            c.weighing.fetch_add(1, Ordering::Relaxed);
            c.return_gallery
        };
        if let Some(w) = self.window() {
            w.global::<App>().set_gallery(back);
        }
        self.publish_export();
        self.publish_bar();
        self.publish_tiles();
        self.request_render();
    }

    fn cancel_export(self: &Rc<Self>) {
        let exporting = self.export.borrow().exporting;
        if exporting {
            self.export.borrow().stop.store(true, Ordering::Relaxed);
        } else {
            self.leave_export();
        }
    }

    /// Esc on the surface is Cancel, or Stop mid-batch.
    pub(super) fn export_escape(self: &Rc<Self>) -> bool {
        let (shown, sheet) = {
            let c = self.export.borrow();
            (c.shown, c.confirm.is_some() || c.notice.is_some() || c.film_row.is_some() || c.papers.shown)
        };
        if sheet {
            self.close_export_sheets();
            return true;
        }
        if shown {
            self.cancel_export();
        }
        shown
    }

    fn close_export_sheets(self: &Rc<Self>) {
        {
            let mut c = self.export.borrow_mut();
            c.confirm = None;
            c.notice = None;
            c.film_row = None;
            c.papers.shown = false;
        }
        self.publish_export();
        self.publish_papers();
    }

    // MARK: - Reading the targets

    /// Every line at once from the sidecars, then each frame decoded once for its size and preview,
    /// in order, off this thread; a newer pass or a space change stops this one.
    fn read_targets(self: &Rc<Self>) {
        let (targets, generation, pass, artist, copyright, rolls) = {
            let c = self.export.borrow();
            let s = self.state.borrow();
            let rolls: Vec<Option<oni_settings::Film>> = c
                .targets
                .iter()
                .map(|t| {
                    let path = t.path.to_string_lossy();
                    s.archive.as_ref().and_then(|a| a.entries().into_iter().find(|e| e.path == path).and_then(|e| e.film.clone()))
                })
                .collect();
            let generation = c.pass.fetch_add(1, Ordering::Relaxed) + 1;
            (c.targets.clone(), generation, c.pass.clone(), c.artist.clone(), c.copyright.clone(), rolls)
        };
        let rows: Vec<Row> = targets.iter().zip(&rolls).map(|(t, roll)| export::opening_row(&t.path, roll.as_ref(), &artist, &copyright)).collect();
        let low = targets.iter().filter(|t| oni_decode::is_low_precision(&t.path)).count();
        {
            let mut c = self.export.borrow_mut();
            // A line typed in before it lands keeps the typing.
            for (i, row) in rows.into_iter().enumerate() {
                if c.rows[i].is_none() {
                    c.rows[i] = Some(row);
                }
            }
            c.low_precision = low;
        }
        self.publish_lines();
        self.publish_export();
        self.render_previews(targets, generation, pass);
    }

    fn render_previews(self: &Rc<Self>, targets: Vec<Target>, generation: u64, pass: Arc<AtomicU64>) {
        let (engine, previews, screen, open) = {
            let s = self.state.borrow();
            (self.engine.clone(), s.previews.clone(), s.screen, s.negative.clone())
        };
        let live = move || pass.load(Ordering::Relaxed) == generation;
        std::thread::Builder::new()
            .name("export previews".into())
            .spawn(move || {
                for (i, target) in targets.into_iter().enumerate() {
                    if !live() {
                        return;
                    }
                    let path = target.path.to_string_lossy().into_owned();
                    let known = open.as_ref().filter(|(p, _)| *p == path).map(|(_, n)| n.clone()).or_else(|| previews.lock().ok()?.get(&path));
                    let negative = match known {
                        Some(n) => Ok(n),
                        None => prepared(&engine, &path, crate::render::Reading::of(&target.settings)),
                    };
                    let size = negative.as_ref().ok().map(|n| {
                        let (w, h) = n.size();
                        export::cropped_size((n.full_width, n.full_width * f64::from(h) / f64::from(w.max(1))), &target.settings)
                    });
                    // One exact render per photo in the export space, as the canvas shows it: proofed.
                    let picture = negative.and_then(|n| {
                        let linear = engine.lock()?.render(&n, &target.settings, Some(PREVIEW_SIDE as usize))?;
                        let k = Params::new(screen, linear.width as u32, linear.height as u32, i as u32);
                        Ok((crate::render::encoded(&linear, &k, 1), linear.width, linear.height))
                    });
                    with_shell(move |c| {
                        if c.export.borrow().pass.load(Ordering::Relaxed) != generation {
                            return;
                        }
                        {
                            let mut card = c.export.borrow_mut();
                            card.sizes[i] = size;
                            card.previews[i] = match picture {
                                Ok((bytes, w, h)) => Preview::Ready(image(&bytes, w, h)),
                                Err(_) => Preview::Failed,
                            };
                        }
                        c.publish_line(i);
                        if i == 0 {
                            c.publish_export();
                            c.weigh_export();
                        }
                    });
                }
            })
            .expect("export preview thread");
    }

    /// `refreshEstimate`: the first target weighed on its 500 px sample, multiplied by the count.
    fn weigh_export(self: &Rc<Self>) {
        let (target, full, export, generation, weighing) = {
            let mut c = self.export.borrow_mut();
            c.estimate = None;
            let generation = c.weighing.fetch_add(1, Ordering::Relaxed) + 1;
            let (Some(target), Some(full)) = (c.targets.first().cloned(), c.full()) else { return };
            (target, full, c.settings.clone(), generation, c.weighing.clone())
        };
        let (engine, previews, open) = {
            let s = self.state.borrow();
            (self.engine.clone(), s.previews.clone(), s.negative.clone())
        };
        std::thread::spawn(move || {
            if weighing.load(Ordering::Relaxed) != generation {
                return;
            }
            let path = target.path.to_string_lossy().into_owned();
            let known = open.as_ref().filter(|(p, _)| *p == path).map(|(_, n)| n.clone()).or_else(|| previews.lock().ok()?.get(&path));
            let negative = known.or_else(|| prepared(&engine, &path, crate::render::Reading::of(&target.settings)).ok());
            let weighed = negative.and_then(|n| export::weigh(&engine, &n, full, &target.settings, &export));
            with_shell(move |c| {
                if c.export.borrow().weighing.load(Ordering::Relaxed) != generation {
                    return;
                }
                c.export.borrow_mut().estimate = weighed;
                c.publish_export();
            });
        });
    }

    // MARK: - The settings

    fn export_changed(self: &Rc<Self>, weigh: bool) {
        self.export.borrow_mut().save_settings();
        self.publish_export();
        self.publish_lines();
        if weigh {
            self.weigh_export();
        }
    }

    /// A new space reaches the canvas, the thumbnails to come and every preview, which render again.
    fn space_changed(self: &Rc<Self>) {
        {
            let c = self.export.borrow();
            if let Ok(mut w) = c.wanted.lock() {
                *w = Proof { colour_only: c.colour_only, ..Proof::new(c.settings.space().clone()) };
            }
        }
        self.export_changed(true);
        self.reproof();
        let shown = self.export.borrow().shown;
        if shown {
            let targets = {
                let mut c = self.export.borrow_mut();
                c.previews = vec![Preview::Waiting; c.targets.len()];
                c.targets.clone()
            };
            self.publish_lines();
            let (generation, pass) = {
                let c = self.export.borrow();
                (c.pass.fetch_add(1, Ordering::Relaxed) + 1, c.pass.clone())
            };
            self.render_previews(targets, generation, pass);
        }
    }

    /// The canvas asked again under the new proof, its settings unchanged.
    fn reproof(self: &Rc<Self>) {
        {
            let mut s = self.state.borrow_mut();
            s.requested = u64::MAX;
            s.requested_live = None;
        }
        self.request_render();
    }

    fn set_space(self: &Rc<Self>, space: Space) {
        {
            let mut c = self.export.borrow_mut();
            c.pending = false;
            if *c.settings.space() == space {
                drop(c);
                return self.publish_export();
            }
            c.settings.set_space(space);
        }
        self.space_changed();
    }

    /// `ExportSpaceField.apply`: Custom lands on the last profile, else the first usable, else waits.
    fn space_tab(self: &Rc<Self>, tab: usize) {
        if let Some(space) = Space::BUILT_IN.get(tab) {
            return self.set_space(space.clone());
        }
        let chosen = {
            let mut c = self.export.borrow_mut();
            if !c.last_profile.is_empty() && OutputProfile::is_printer_at(Path::new(&c.last_profile)) {
                Some(OutputProfile::new(c.last_profile.clone()))
            } else if let Some(first) = c.installed.iter().find(|p| p.is_usable()).cloned() {
                c.last_profile = first.path.clone();
                let path = first.path.clone();
                c.save(LAST_PROFILE, Value::from(path));
                Some(first)
            } else {
                c.pending = true;
                None
            }
        };
        match chosen {
            Some(p) => self.set_space(Space::Profile(p)),
            None => self.publish_export(),
        }
    }

    pub fn choose_profile(self: &Rc<Self>, profile: OutputProfile) {
        {
            let mut c = self.export.borrow_mut();
            c.last_profile = profile.path.clone();
            c.save(LAST_PROFILE, Value::from(profile.path.clone()));
        }
        self.set_space(Space::Profile(profile));
    }

    fn ask_paths(&self, what: &Ask) -> Vec<PathBuf> {
        let chooser = self.export.borrow().chooser.clone();
        if let Some(chooser) = chooser {
            return chooser(what);
        }
        match what {
            Ask::Save { name, extension } => crate::system::save_file("Export", name, &[extension.as_str()]).into_iter().collect(),
            Ask::Folder { count } => crate::system::pick_folder(&format!("Where should the {count} files be written?")).into_iter().collect(),
            Ask::Profiles => crate::system::pick_files("Choose ICC profiles, or the zip a paper maker or a lab ships them in.", &["icc", "icm", "zip"]),
        }
    }

    /// `loadProfile`: the files installed into the user's profiles, the first chosen, the rest reported.
    pub fn load_profile(self: &Rc<Self>) {
        let paths = self.ask_paths(&Ask::Profiles);
        if paths.is_empty() {
            return;
        }
        let (folder, scratch) = {
            let c = self.export.borrow();
            (c.profiles.clone(), c.support.join("Scratch"))
        };
        let (installed, refused) = OutputProfile::importing(&paths, &folder, &scratch);
        let list = OutputProfile::installed(&OutputProfile::roots(&folder), true);
        self.export.borrow_mut().installed = list;
        if let Some(first) = installed.first() {
            self.choose_profile(first.clone());
        }
        if let Some(notice) = export::import_report(installed.len(), refused) {
            self.export.borrow_mut().notice = Some(notice);
        }
        self.publish_export();
    }

    /// The profiles installed, walked off this thread at launch, as the field's task walks them.
    pub(super) fn list_profiles(&self) {
        let folder = self.export.borrow().profiles.clone();
        std::thread::spawn(move || {
            let list = OutputProfile::installed(&OutputProfile::roots(&folder), true);
            with_shell(move |c| {
                c.export.borrow_mut().installed = list;
                c.publish_export();
            });
        });
    }

    fn set_colour_only(self: &Rc<Self>, on: bool) {
        {
            let mut c = self.export.borrow_mut();
            c.colour_only = on;
            c.save(PROOF_COLOUR_ONLY, Value::from(on));
        }
        self.space_changed();
    }

    // MARK: - The quality slider

    fn quality_fraction(q: f64) -> f32 {
        ((q - QUALITY.0) / (QUALITY.1 - QUALITY.0)) as f32
    }

    fn set_quality(self: &Rc<Self>, q: f64, weigh: bool) {
        self.export.borrow_mut().settings.jpeg_quality = q.clamp(QUALITY.0, QUALITY.1);
        self.export_changed(weigh);
    }

    // MARK: - Writing

    /// `runExport`: one file asks for its name, a batch for a folder and then for any clash.
    pub fn run_export(self: &Rc<Self>) {
        let (targets, rows, format) = {
            let c = self.export.borrow();
            (c.targets.clone(), c.rows.clone(), c.settings.format())
        };
        if targets.is_empty() || self.export.borrow().exporting {
            return;
        }
        if targets.len() == 1 {
            let typed = rows[0].as_ref().map_or("", |r| r.name.as_str());
            let name = oni_export::name(&targets[0].path, typed, format);
            let Some(destination) = self.ask_paths(&Ask::Save { name, extension: format.file_extension().into() }).into_iter().next() else { return };
            return self.write_export(vec![destination]);
        }
        let Some(folder) = self.ask_paths(&Ask::Folder { count: targets.len() }).into_iter().next() else { return };
        let destinations = export::destinations(&folder, &targets, &rows, format);
        // A folder picker does not ask before overwriting, so the batch checks its clashes itself.
        let clashes = oni_export::clashes(&destinations, |p| p.exists());
        if let Some((title, message)) = export::clash_notice(clashes.len(), destinations.len()) {
            {
                let mut c = self.export.borrow_mut();
                c.confirm = Some(destinations);
                c.notice = None;
            }
            if let Some(w) = self.window() {
                let e = w.global::<Exporting>();
                e.set_confirm_title(title.into());
                e.set_confirm_message(message.into());
            }
            return self.publish_export();
        }
        self.write_export(destinations);
    }

    fn confirmed(self: &Rc<Self>, replace: bool) {
        let chosen = self.export.borrow_mut().confirm.take();
        self.publish_export();
        if replace && let Some(destinations) = chosen {
            self.write_export(destinations);
        }
    }

    /// `writeExport`: in order off this thread, the stop honoured between files; the first line's
    /// artist and copyright are remembered for the next photo's empty line.
    fn write_export(self: &Rc<Self>, destinations: Vec<PathBuf>) {
        let (plan, export, stop) = {
            let mut c = self.export.borrow_mut();
            c.exporting = true;
            c.progress = 0;
            c.stopped = None;
            c.stop.store(false, Ordering::Relaxed);
            if let Some(Some(first)) = c.rows.first().cloned() {
                c.artist = first.metadata.artist.clone();
                c.copyright = first.metadata.copyright.clone();
                c.save(ARTIST, Value::from(first.metadata.artist));
                c.save(COPYRIGHT, Value::from(first.metadata.copyright));
            }
            let plan: Vec<(Target, PathBuf, ExifMetadata)> = c
                .targets
                .iter()
                .zip(destinations)
                .zip(&c.rows)
                .map(|((t, d), r)| (t.clone(), d, r.as_ref().map(|r| r.metadata.clone()).unwrap_or_default()))
                .collect();
            (plan, c.settings.clone(), c.stop.clone())
        };
        self.publish_export();
        let engine: Shared = self.engine.clone();
        let journal = Journal::in_support(&self.export.borrow().support);
        std::thread::Builder::new()
            .name("export".into())
            .spawn(move || {
                for (target, destination, metadata) in plan {
                    if stop.load(Ordering::Relaxed) {
                        return with_shell(|c| {
                            {
                                let mut card = c.export.borrow_mut();
                                card.exporting = false;
                                card.stopped = Some(card.progress);
                            }
                            c.publish_export();
                        });
                    }
                    match export::write_one(&engine, &target, &destination, &export, &metadata) {
                        Ok(()) => {
                            let subject = target.path.to_string_lossy().into_owned();
                            journal.note(Gesture::Exported, Some(&subject), 0, 0, None, oni_settings::Date::now());
                            with_shell(|c| {
                                c.export.borrow_mut().progress += 1;
                                c.publish_export();
                            });
                        }
                        Err(e) => {
                            let name = target.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                            let message = format!("{name}: {e}");
                            return with_shell(move |c| {
                                {
                                    let mut card = c.export.borrow_mut();
                                    card.exporting = false;
                                    card.notice = Some(("Export Failed".into(), message));
                                }
                                c.publish_export();
                            });
                        }
                    }
                }
                with_shell(|c| {
                    c.export.borrow_mut().exporting = false;
                    c.leave_export();
                });
            })
            .expect("export thread");
    }

    // MARK: - Publishing

    fn publish_lines(&self) {
        let Some(w) = self.window() else { return };
        let lines: Vec<ExportLine> = {
            let c = self.export.borrow();
            (0..c.targets.len()).map(|i| c.line(i)).collect()
        };
        let model = self.export.borrow().model.clone();
        if model.row_count() == lines.len() {
            for (i, l) in lines.into_iter().enumerate() {
                model.set_row_data(i, l);
            }
        } else {
            model.set_vec(lines);
        }
        w.global::<Exporting>().set_lines(ModelRc::from(model));
    }

    fn publish_line(&self, i: usize) {
        let line = {
            let c = self.export.borrow();
            (i < c.targets.len()).then(|| c.line(i))
        };
        let model = self.export.borrow().model.clone();
        if let Some(line) = line
            && i < model.row_count()
        {
            model.set_row_data(i, line);
        }
    }

    /// Every property the surface, the chip and the menus read, read before any is set.
    pub(super) fn publish_export(&self) {
        let Some(w) = self.window() else { return };
        let (selected, has_photo) = {
            let s = self.state.borrow();
            (s.picks.selection.len(), s.open.is_some())
        };
        let c = self.export.borrow();
        let e = w.global::<Exporting>();
        let space = c.settings.space().clone();
        let count = c.targets.len();
        let batch = count > 1;
        e.set_shown(c.shown);
        e.set_on_paper(space.paper().is_some());
        e.set_count(count as i32);
        e.set_subtitle(export::subtitle(&c.targets, c.exporting, c.progress, c.stopped).into());
        e.set_extension(format!(".{}", c.settings.format().file_extension()).into());
        e.set_format(Format::ALL.iter().position(|f| *f == c.settings.format()).unwrap_or(0) as i32);
        let on_profile = matches!(space, Space::Profile(_));
        e.set_space_tab(if on_profile || c.pending { 4 } else { Space::BUILT_IN.iter().position(|s| *s == space).unwrap_or(1) as i32 });
        e.set_purpose(space.purpose().into());
        e.set_jpeg(c.settings.format() == Format::Jpeg);
        e.set_on_profile(on_profile);
        e.set_pending(c.pending && !on_profile);
        e.set_pending_note(format!("Until a profile is chosen, the picture stays in {}.", space.title()).into());
        if let Space::Profile(chosen) = &space {
            let (listed, titles) = c.profile_titles(chosen);
            let at = listed.iter().position(|p| p == chosen).unwrap_or(0);
            e.set_profiles(ModelRc::new(VecModel::from(titles.into_iter().map(SharedString::from).collect::<Vec<_>>())));
            e.set_profile(at as i32);
            e.set_profile_note(export::profile_note(chosen, &c.installed).into());
        }
        e.set_colour_only(c.colour_only);
        e.set_edited_in(export::edited_in_note(c.edited_in.as_ref(), &space, batch).into());
        e.set_compress(c.settings.compress_tiff);
        e.set_quality_fraction(Ctl::quality_fraction(c.settings.jpeg_quality));
        e.set_quality_readout(format!("{:.0}%", c.settings.jpeg_quality * 100.0).into());
        let (wt, ht) = export::size_texts(c.full(), c.settings.resize);
        e.set_width_text(wt.into());
        e.set_height_text(ht.into());
        e.set_size_enabled(c.full().is_some());
        e.set_resized(c.settings.resize != Resize::Full);
        e.set_hdr(c.settings.hdr);
        e.set_bits(export::bits(c.settings.format()).into());
        let dims = c.full().map(|(fw, fh)| {
            let (ow, oh) = c.settings.resize.output_size(fw, fh);
            format!("{} × {}", ow as i64, oh as i64)
        });
        e.set_dims(dims.unwrap_or_else(|| "—".into()).into());
        e.set_weight_label(if batch { "estimated total size" } else { "estimated size" }.into());
        e.set_weight(export::weight(c.estimate, count).into());
        e.set_low_precision(export::low_precision(c.low_precision, count).into());
        e.set_exporting(c.exporting);
        e.set_can_export(count > 0);
        e.set_key_title(label::key(count).into());
        e.set_menu_title(label::menu(if c.shown { count } else { selected }).into());
        e.set_menu_live(if c.shown { count > 0 && !c.exporting } else { has_photo || selected > 0 });
        e.set_gamut_title(if c.gamut { "Hide What This Screen Cannot Show" } else { "Show What This Screen Cannot Show" }.into());
        e.set_chip_title(space.title().into());
        e.set_chip_hint(space.hint().into());
        e.set_confirm_shown(c.confirm.is_some());
        e.set_notice_shown(c.notice.is_some());
        if let Some((title, message)) = &c.notice {
            e.set_notice_title(title.as_str().into());
            e.set_notice_message(message.as_str().into());
        }
        e.set_stocks_shown(c.film_row.is_some());
        drop(c);
        self.publish_menus();
    }

    // MARK: - Wiring

    /// The proof every render the editor shows goes through, and the profiles the field lists.
    pub(super) fn attach_export(&self) {
        let wanted = self.export.borrow().wanted.clone();
        if let Ok(mut engine) = self.engine.lock() {
            engine.proof.wanted = Some(wanted);
        }
        self.list_profiles();
        self.publish_export();
    }

    /// The stripes as the last session left them.
    pub(super) fn restore_gamut(self: &Rc<Self>) {
        if self.export.borrow().gamut && !self.state.borrow().screen_gamut {
            self.toggle_screen_gamut();
        }
    }

    /// The File menu's item, which counts the selection off the surface.
    pub(super) fn publish_export_menu(&self, selected: usize, has_photo: bool) {
        let Some(w) = self.window() else { return };
        let c = self.export.borrow();
        if c.shown {
            return;
        }
        let e = w.global::<Exporting>();
        e.set_menu_title(label::menu(selected).into());
        e.set_menu_live(has_photo || selected > 0);
    }

    pub(super) fn wire_export(me: &Rc<Ctl>, window: &MainWindow) {
        let e = window.global::<Exporting>();
        let p = me.clone();
        e.on_preset(move |i| {
            let Some(preset) = Preset::ALL.get(i as usize).copied() else { return };
            let before = p.export.borrow().settings.space().clone();
            {
                let mut c = p.export.borrow_mut();
                c.settings = preset.applied(&c.settings);
            }
            if *p.export.borrow().settings.space() != before {
                p.space_changed();
            } else {
                p.export_changed(true);
            }
        });
        let p = me.clone();
        e.on_format_picked(move |i| {
            let Some(format) = Format::ALL.get(i as usize).copied() else { return };
            let before = p.export.borrow().settings.space().clone();
            p.export.borrow_mut().settings.set_format(format);
            if *p.export.borrow().settings.space() != before {
                p.space_changed();
            } else {
                p.export_changed(true);
            }
        });
        let p = me.clone();
        e.on_space_picked(move |i| p.space_tab(i.max(0) as usize));
        let p = me.clone();
        e.on_profile_picked(move |i| {
            let chosen = {
                let c = p.export.borrow();
                let Space::Profile(current) = c.settings.space() else { return };
                c.profile_titles(current).0.get(i.max(0) as usize).cloned()
            };
            if let Some(profile) = chosen {
                p.choose_profile(profile);
            }
        });
        let p = me.clone();
        e.on_pending_picked(move |i| {
            if i == 1 {
                p.load_profile();
            }
        });
        let p = me.clone();
        e.on_load_profile(move || p.load_profile());
        let p = me.clone();
        Ctl::wire_papers(&p, &e);
        let p = me.clone();
        e.on_colour_only_set(move |on| p.set_colour_only(on));
        let p = me.clone();
        e.on_compress_set(move |on| {
            p.export.borrow_mut().settings.compress_tiff = on;
            p.export_changed(true);
        });
        let p = me.clone();
        e.on_hdr_set(move |on| {
            p.export.borrow_mut().settings.hdr = on;
            p.export_changed(true);
        });
        let p = me.clone();
        e.on_quality_press(move |_fine| {
            let now = Instant::now();
            let double = {
                let mut c = p.export.borrow_mut();
                let double = c.quality.last_press.is_some_and(|t| now.duration_since(t) <= DOUBLE_CLICK);
                c.quality.last_press = Some(now);
                c.quality.start = if double { None } else { Some(Ctl::quality_fraction(c.settings.jpeg_quality)) };
                double
            };
            if double {
                p.set_quality(QUALITY_BASE, true);
            }
            double
        });
        let p = me.clone();
        e.on_quality_drag(move |translation, span, fine| {
            let Some(start) = p.export.borrow().quality.start else { return };
            let at = track::clamped(track::asked(start, translation, span, fine));
            p.set_quality(QUALITY.0 + f64::from(at) * (QUALITY.1 - QUALITY.0), false);
        });
        let p = me.clone();
        e.on_quality_release(move || {
            p.export.borrow_mut().quality.start = None;
            p.weigh_export();
        });
        let p = me.clone();
        e.on_quality_step(move |d| {
            let q = p.export.borrow().settings.jpeg_quality + f64::from(d) * (QUALITY.1 - QUALITY.0) / 100.0;
            p.set_quality(q, true);
        });
        let p = me.clone();
        e.on_size_typed(move |width, height| {
            let resize = {
                let c = p.export.borrow();
                let Some(full) = c.full() else { return };
                let (sw, sh) = export::size_texts(Some(full), c.settings.resize);
                export::typed_size(&width, &height, (&sw, &sh), full)
            };
            if let Some(resize) = resize {
                p.export.borrow_mut().settings.resize = resize;
            }
            p.export_changed(resize.is_some());
        });
        let p = me.clone();
        e.on_full_size(move || {
            p.export.borrow_mut().settings.resize = Resize::Full;
            p.export_changed(true);
        });
        let p = me.clone();
        e.on_edited(move |row, field, text| {
            let mut c = p.export.borrow_mut();
            let Some(Some(r)) = c.rows.get_mut(row.max(0) as usize) else { return };
            let text = text.to_string();
            match field.as_str() {
                "name" => r.name = text,
                "camera" => r.metadata.camera = text,
                "lens" => r.metadata.lens = text,
                "iso" => r.metadata.iso = text,
                "date" => r.metadata.date = text,
                "artist" => r.metadata.artist = text,
                "copyright" => r.metadata.copyright = text,
                _ => {}
            }
        });
        let p = me.clone();
        e.on_pick_film(move |row| {
            let current = {
                let mut c = p.export.borrow_mut();
                c.film_row = Some(row.max(0) as usize);
                let film = c.rows.get(row.max(0) as usize).cloned().flatten().map(|r| r.metadata.film).unwrap_or_default();
                FILM_STOCKS.iter().position(|k| k.display_name == film).map_or(-1, |i| i as i32)
            };
            if let Some(w) = p.window() {
                let e = w.global::<Exporting>();
                e.set_stocks(ModelRc::new(VecModel::from(FILM_STOCKS.iter().map(|k| SharedString::from(k.display_name)).collect::<Vec<_>>())));
                e.set_stock(current);
            }
            p.publish_export();
        });
        let p = me.clone();
        e.on_stock_chosen(move |i| {
            let row = p.export.borrow_mut().film_row.take();
            if let (Some(row), Some(kind)) = (row, usize::try_from(i).ok().and_then(|i| FILM_STOCKS.get(i)))
                && let Some(Some(r)) = p.export.borrow_mut().rows.get_mut(row)
            {
                r.metadata.film = kind.display_name.to_owned();
            }
            if let Some(row) = row {
                p.publish_line(row);
            }
            p.publish_export();
        });
        let p = me.clone();
        e.on_cancel(move || p.cancel_export());
        let p = me.clone();
        e.on_export(move || p.run_export());
        let p = me.clone();
        e.on_confirmed(move |replace| p.confirmed(replace));
        // `PaperCorrection.automatic`: the paper's own white, read off its profile alone.
        let p = me.clone();
        e.on_auto_paper(move || {
            let Some(paper) = p.export.borrow().settings.space().paper().cloned() else { return };
            let Some(found) = oni_colour::paper::automatic(&paper) else { return };
            let next = {
                let s = p.state.borrow();
                let mut next = s.store.settings().clone();
                next.paper_correction = found;
                next
            };
            p.write(oni_core::Command::Write(Box::new(next)));
        });
        let p = me.clone();
        e.on_notice_closed(move || {
            p.export.borrow_mut().notice = None;
            p.publish_export();
        });
    }
}

/// The surface's state, read back by the checks.
pub fn card(ctl: &Ctl) -> std::cell::Ref<'_, Card> {
    ctl.export.borrow()
}

/// Lines of the surface as Slint was last given them, read back by the checks.
pub fn lines(window: &MainWindow) -> Vec<ExportLine> {
    let model = window.global::<Exporting>().get_lines();
    (0..model.row_count()).filter_map(|i| model.row_data(i)).collect()
}

/// A check stands in for the system's panels.
pub fn answer_with(ctl: &Ctl, chooser: Chooser) {
    ctl.export.borrow_mut().chooser = Some(chooser);
}

/// A check installs its profiles in its own folder, never the user's ColorSync one.
pub fn profiles_in(ctl: &Ctl, folder: PathBuf) {
    ctl.export.borrow_mut().profiles = folder;
}
