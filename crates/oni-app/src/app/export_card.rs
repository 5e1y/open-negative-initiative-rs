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
use oni_library::journal::Kind as Gesture;
use oni_settings::film::FILM_STOCKS;
use serde_json::Value;
use slint::{ComponentHandle, Image, Model, ModelRc, SharedString, VecModel};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Instant;

mod entering;
mod targets;
mod settings;
mod quality;
mod writing;
pub mod writer;
mod contact;
mod dated;
mod sheet_view;
mod publishing;
mod wiring;
mod kept;
mod notes;

pub mod job;
pub mod paper_sheet;
pub use kept::{Kept, kept};

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
    /// A batch written as one contact sheet, as Edit Together lays it.
    pub sheet: bool,
    pub colour_only: bool,
    pub gamut: bool,
    pub targets: Vec<Target>,
    pub rows: Vec<Option<Row>>,
    /// The lines whose camera or lens a hand typed or picked here: the scan's own name the digitiser's.
    pub typed_gear: std::collections::HashSet<(usize, oni_library::collection::Gear)>,
    sizes: Vec<Option<(f64, f64)>>,
    previews: Vec<Preview>,
    /// The first target's reduced decode, as its preview read it, which the estimate's sample renders.
    first: Option<Arc<crate::render::Negative>>,
    /// The room the column gives the sheet, in points, and its logo band last drawn, for its width.
    sheet_room: Option<(f64, f64)>,
    logo: Option<(usize, Image)>,
    /// The room a line's preview takes, once the column is laid out, which sizes each render.
    room: Arc<std::sync::Mutex<Option<targets::Room>>>,
    low_precision: usize,
    estimate: Option<u64>,
    pub exporting: bool,
    pub progress: usize,
    stopped: Option<usize>,
    pub writer: writer::Writer,
    /// Each batch's pass and each estimate carry a generation; a newer one makes them stop.
    pass: Arc<AtomicU64>,
    weighing: Arc<AtomicU64>,
    /// The lines' reading, its own: a preview pass started again leaves the lines to land.
    reading: Arc<AtomicU64>,
    /// The estimate weighed on a thread of its own, on the latest settings asked.
    weigher: crate::latest::Worker<targets::Weigh>,
    /// The weighings run, read back by the checks.
    weighed: Arc<AtomicUsize>,
    /// The settings kept on a thread of its own, the latest asked written once.
    saver: crate::latest::Worker<String>,
    /// The presets the hand saved, kept as the settings are, and the name typed for the next one.
    kept: Vec<Kept>,
    kept_saver: crate::latest::Worker<Value>,
    /// The name sheet open, and the name typed in it.
    naming: Option<kept::Naming>,
    draft: String,
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
    /// The clash dialog's files and whether its key replaces earlier exports.
    confirm: Option<(Vec<PathBuf>, bool)>,
    quality: QualityHand,
    film_row: Option<usize>,
    pub notice: Option<(String, String)>,
    /// The export under way or the last one ended, which the status bar follows.
    pub job: Option<job::Job>,
    /// The engine exports render on, apart from the editor's.
    apart: Option<Shared>,
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
        let weighed = Arc::new(AtomicUsize::new(0));
        let kept = support.to_path_buf();
        let saver = crate::latest::Worker::spawn("export settings", move |raw: String| {
            let _ = Preferences::load(&kept).set(oni_export::settings::PREFERENCE_KEY, Value::from(raw));
        });
        let folder = support.to_path_buf();
        let kept_saver = crate::latest::Worker::spawn("export presets", move |presets: Value| {
            let _ = Preferences::load(&folder).set(kept::KEPT, presets);
        });
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
            sheet: false,
            targets: Vec::new(),
            rows: Vec::new(),
            typed_gear: Default::default(),
            sizes: Vec::new(),
            previews: Vec::new(),
            first: None,
            sheet_room: None,
            logo: None,
            room: Arc::default(),
            low_precision: 0,
            estimate: None,
            exporting: false,
            progress: 0,
            stopped: None,
            writer: writer::Writer::default(),
            pass: Arc::default(),
            weighing: Arc::default(),
            reading: Arc::default(),
            weigher: targets::weigher(weighed.clone()),
            weighed,
            saver,
            kept: kept::from_value(prefs.value(kept::KEPT)),
            kept_saver,
            naming: None,
            draft: String::new(),
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
            job: None,
            apart: None,
            model: Rc::new(VecModel::default()),
            papers: paper_sheet::Sheet::default(),
        }
    }

    /// The first target's full size after its turns and crop, once measured; on a contact sheet, the
    /// sheet's once every target is.
    fn full(&self) -> Option<(f64, f64)> {
        if self.on_sheet() {
            return self.sizes.iter().copied().collect::<Option<Vec<_>>>().map(|sizes| super::gallery_view::together::sheet_size(&sizes));
        }
        self.sizes.first().copied().flatten()
    }

    /// What the estimate weighs: the first target's scale in the file, on a sheet its tile's, and the
    /// file's pixels, the size the panel shows.
    fn written(&self) -> Option<(f64, f64)> {
        let (fw, fh) = self.full()?;
        let resize = self.settings.resize;
        let (w, h) = resize.output_size(fw, fh);
        let mut scale = resize.scale(fw, fh);
        if self.on_sheet() {
            let first = self.sizes.first().copied().flatten()?;
            let (pictures, _, _) = super::gallery_view::together::sheet_places(&self.sizes);
            let &(_, _, pw, ph) = pictures.first()?;
            scale *= pw.max(ph) / first.0.max(first.1);
        }
        Some((scale, w * h))
    }

    /// Whether the batch is written as one contact sheet: two photos or more.
    pub fn on_sheet(&self) -> bool {
        self.sheet && self.targets.len() > 1
    }

    fn save(&mut self, key: &str, value: Value) {
        let _ = self.prefs.set(key, value);
    }

    /// The settings kept off this thread: a write costs the disk's flush, a burst of changes one write.
    fn save_settings(&mut self) {
        self.saver.ask(self.settings.raw_value());
    }

    fn line(&self, i: usize) -> ExportLine {
        let row = self.rows.get(i).cloned().flatten();
        let m = row.as_ref().map(|r| r.metadata.clone()).unwrap_or_default();
        let target = &self.targets[i];
        let (preview, state) = match self.previews.get(i) {
            Some(Preview::Ready(img)) => (img.clone(), crate::PreviewState::Shown),
            Some(Preview::Failed) => (Image::default(), crate::PreviewState::Failed),
            _ => (Image::default(), crate::PreviewState::Rendering),
        };
        let size = self.sizes.get(i).copied().flatten();
        let caption = size.map(|(w, h)| oni_settings::geometry::pixel_label(self.settings.resize.output_size(w, h)));
        ExportLine {
            name: row.as_ref().map_or_else(|| oni_export::stem(&target.path), |r| r.name.clone()).into(),
            camera: m.camera.into(),
            lens: m.lens.into(),
            film: m.film.into(),
            iso: m.iso.into(),
            date: dated::shown(&m.date).into(),
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



/// The surface's state, read back by the checks.
pub fn card(ctl: &Ctl) -> std::cell::Ref<'_, Card> {
    ctl.export.borrow()
}

/// The estimate shown, `None` while one is weighed, and the weighings run, read back by the checks.
pub fn estimate(ctl: &Ctl) -> (Option<u64>, usize) {
    let c = ctl.export.borrow();
    (c.estimate, c.weighed.load(Ordering::SeqCst))
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
