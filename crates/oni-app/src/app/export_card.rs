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
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

mod entering;
mod targets;
mod settings;
mod quality;
mod writing;
mod publishing;
mod wiring;

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
    /// The room a line's preview takes, once the column is laid out, which sizes each render.
    room: Arc<std::sync::Mutex<Option<targets::Room>>>,
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
            room: Arc::default(),
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
