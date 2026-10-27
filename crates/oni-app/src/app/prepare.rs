//! Prepare the Roll (`.roll`): five steps over the roll's frames, a draft in every sidecar, the
//! measure off this thread with its progress and Stop, landed as one gallery step, then the editor.

use super::sheets::{self, Sheet};
use super::{Ctl, State, frames, position, stored_at, with_shell};
use crate::fields::Field;
use crate::framing::Handle;
use crate::roll_previews::{Bitmap, Look, Out, Previews};
use crate::{App, MenuLine, Preparing, RollCell};
use oni_i18n::{tr, trn};
use oni_library::capture::CaptureExposure;
use oni_library::{rgb_passes, roll_membership};
use oni_measure::{key, pipette, roll};
use oni_settings::cg::{Point, Rect};
use oni_settings::frame::{FlatField, Geometry, PassOrder, RgbPasses};
use oni_settings::sidecar::{self, DraftBase, RollDraft};
use oni_settings::{ConversionMode, Id, PipelineSettings};
use slint::{ComponentHandle, Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Instant;

mod passes;
mod model;
mod wiring;
mod entering;
mod steps;
mod sliders;
mod card;
mod measure;
mod previews;
mod publishing;
mod flat_masks;
mod tutorial;
mod draft_base;
mod trace;
mod negative;
mod slides;
pub use model::*;
pub use slides::{last_step, slides_title};

/// `RollBasePick`: the bare film's point on a scan, origin top left, and the disc's radius.
#[derive(Clone, Debug, PartialEq)]
pub struct BasePick {
    pub path: String,
    pub unit: (f64, f64),
    pub radius: f64,
}

/// What the measure's thread hands back.
type Written = Vec<oni_core::store::roll::WrittenFrame>;

type Job = Box<dyn FnOnce() + Send>;

/// The longest a close waits for the drafts: a roll's sidecars take milliseconds each on a sound drive.
const DRAFT_FLUSH: std::time::Duration = std::time::Duration::from_secs(10);

/// What the measure's sheet is named after and may replace, read off the rows on this thread.
struct SheetPlan {
    film: Option<oni_settings::Film>,
    earliest: oni_settings::Date,
    /// The gallery group of the first frame: a sheet of the roll's former reference there is rewritten.
    folder: Vec<String>,
}

impl SheetPlan {
    fn of(rows: &[oni_library::Entry], archive: Option<&crate::library::Archive>, picked: &[Frame]) -> SheetPlan {
        let known: Vec<&oni_library::Entry> = picked.iter().filter_map(|f| rows.iter().find(|e| e.path == f.path)).collect();
        let film = known.iter().find_map(|e| e.film.clone());
        let earliest =
            known.iter().map(|e| oni_library::film_group::capture(e)).reduce(|a, b| if b < a { b } else { a }).unwrap_or_else(oni_settings::Date::now);
        let first = picked.first().map(|f| f.path.as_str());
        let folder = archive
            .and_then(|a| a.groups.iter().find(|g| g.entries.iter().any(|e| Some(e.path.as_str()) == first)))
            .map(|g| g.entries.iter().map(|e| e.path.clone()).collect())
            .unwrap_or_default();
        SheetPlan { film, earliest, folder }
    }

    /// The sheet laid from the measure's tiles and written over the roll's former sheet, else beside
    /// the first frame; `None` when nothing lays or the file cannot be written.
    fn written(&self, paths: &[String], jobs: &[roll::Job], tiles: &[(String, oni_measure::sheet::Picture)], reads: &[Option<Rect>]) -> Option<Sheet> {
        let (layout, pixels) = sheets::laid(tiles, paths.first()?, oni_measure::sheet::roll_tile(), reads)?;
        let former: HashSet<Id> = jobs.iter().filter_map(|j| j.before.roll_reference.as_ref().map(|r| r.id)).collect();
        let replaced = self.folder.iter().find(|p| !paths.contains(p) && roll_membership::sheet_reference(p).is_some_and(|id| former.contains(&id)));
        let stem = sheets::named(self.film.as_ref(), self.earliest);
        let to = match replaced {
            Some(p) => sheets::To::Over(Path::new(p)),
            None => sheets::To::Beside { lead: Path::new(&paths[0]), stem: &stem },
        };
        let destination = sheets::write(&layout, &pixels, to).ok()?;
        let mosaic = tiles.iter().all(|(p, _)| oni_decode::decodes_via_mosaic(Path::new(p)));
        Some(Sheet { path: destination, grid: layout.grid, film: self.film.clone(), stands: sheets::stands(self.earliest), mosaic })
    }
}

/// The roll being prepared: `mode == .roll` and every `roll…` value of the scene.
pub(crate) struct Roll {
    pub shown: bool,
    back_to_gallery: bool,
    frames: Vec<Frame>,
    /// `rollSources`: the files as they came, which a grouping of passes reads.
    sources: Vec<Frame>,
    pass_offset: usize,
    grouping: Option<rgb_passes::Grouping>,
    pass_pictures: HashMap<String, Image>,
    passes_asked: u64,
    read: HashMap<String, PipelineSettings>,
    left_out: usize,
    window: Rect,
    aspect: f64,
    flat: FlatField,
    film: ConversionMode,
    capture: Capture,
    pass_order: PassOrder,
    base: Option<BasePick>,
    step: Step,
    at: usize,
    flat_pick: Option<String>,
    /// `FlatFieldBlock.loading`: a picture chosen by Load… being decoded as a mask.
    loading_mask: bool,
    base_frame: usize,
    progress: Option<(usize, usize)>,
    stop: Arc<AtomicBool>,
    draft_id: Option<Id>,
    draftless: HashSet<String>,
    /// Bumped at each entry and landing, so a late post of a session gone lands nowhere.
    session: u64,
    previews: Option<Previews>,
    serial: Option<mpsc::Sender<Job>>,
    tiles: HashMap<String, Image>,
    card: Option<(String, Image)>,
    /// The base's scan, its size, and its size at full resolution.
    scan: Option<(String, Image, (usize, usize), (usize, usize))>,
    /// The loupe's full resolutions, of the base frame and its neighbours, and the one asked since when.
    pub(in crate::app) fulls: Vec<(String, Image, (usize, usize))>,
    asked_full: Option<(String, Instant)>,
    pointer: Option<(f64, f64)>,
    asked_scan: Option<String>,
    shown_path: Option<String>,
    window_gesture: Option<(Handle, Rect, (f64, f64))>,
    slide: Option<(usize, f32)>,
    /// The Picker Size the hand holds, written to the preferences once the slider is let go.
    picker: Option<f32>,
    last_press: Option<(usize, Instant)>,
    asked: HashSet<String>,
    /// `work.openedAmong`: the selection a load narrowed, when it held the frame opened.
    pub opened_among: HashSet<String>,
    pub require: bool,
    tutorial: Option<tutorial::Playing>,
    trace: trace::Trace,
    /// What the scan and the loupe were last given, by path and size: a 24 MP picture set again
    /// would be compared pixel by pixel at every move of the pointer.
    laid_scan: Option<(String, (usize, usize))>,
    laid_loupe: Option<(String, (usize, usize))>,
    /// Each cell's photo and picture source (0 none, 1 thumbnail, 2 tile) as last laid, and the tiles
    /// landed since: only those cells are written again, never compared pixel by pixel.
    laid_cells: Vec<(String, u8)>,
    fresh_tiles: HashSet<String>,
}

impl Default for Roll {
    fn default() -> Self {
        Roll {
            shown: false,
            back_to_gallery: true,
            frames: Vec::new(),
            sources: Vec::new(),
            pass_offset: 0,
            grouping: None,
            pass_pictures: HashMap::new(),
            passes_asked: 0,
            read: HashMap::new(),
            left_out: 0,
            window: oni_measure::window::proposal(&Rect::UNIT, false),
            aspect: 1.5,
            flat: FlatField::default(),
            film: ConversionMode::Negative,
            capture: Capture::Camera,
            pass_order: PassOrder::Bgr,
            base: None,
            step: Step::Film,
            at: 0,
            flat_pick: None,
            loading_mask: false,
            base_frame: 0,
            progress: None,
            stop: Arc::default(),
            draft_id: None,
            draftless: HashSet::new(),
            session: 0,
            previews: None,
            serial: None,
            tiles: HashMap::new(),
            card: None,
            scan: None,
            fulls: Vec::new(),
            asked_full: None,
            pointer: None,
            asked_scan: None,
            shown_path: None,
            window_gesture: None,
            slide: None,
            picker: None,
            last_press: None,
            asked: HashSet::new(),
            opened_among: HashSet::new(),
            require: false,
            tutorial: None,
            trace: trace::Trace::default(),
            laid_scan: None,
            laid_loupe: None,
            laid_cells: Vec::new(),
            fresh_tiles: HashSet::new(),
        }
    }
}

impl Roll {
    pub(super) fn new(require: bool) -> Roll {
        Roll { require, ..Roll::default() }
    }

    /// The frames whose preview has landed, which a check waits for before reading one.
    pub fn previewed(&self) -> usize {
        self.tiles.len()
    }

    /// The roll shown, by its frames' paths.
    fn roll(&self) -> Vec<String> {
        self.sources.iter().map(|f| f.path.clone()).collect()
    }

    /// Every frame's sidecar read, which the bench waits for.
    pub fn read_whole(&self) -> bool {
        !self.sources.is_empty() && self.read.len() >= self.sources.len()
    }

    /// `refreshRollPreviews`: every preview dropped and asked again for the frames the roll now has.
    fn refresh_previews(&mut self) {
        self.tiles.clear();
        self.card = None;
        self.shown_path = None;
        let Some(p) = &self.previews else { return };
        p.reset(self.session);
        for f in &self.frames {
            if let Some(read) = self.read.get(&f.path) {
                p.add(&f.path, read);
            }
        }
    }

    /// `rollFlatField`: none while neutral or on a scanner.
    fn mask(&self) -> Option<FlatField> {
        (!self.flat.is_neutral() && self.capture != Capture::Scanner).then(|| self.flat.clone())
    }

    fn look(&self) -> Look {
        Look { film: self.film, flat: self.mask() }
    }

    /// `rollShownFrame`.
    fn shown_frame(&self) -> Option<&Frame> {
        if self.step == Step::FlatField
            && let Some(pick) = &self.flat_pick
        {
            return self.frames.iter().find(|f| &f.path == pick);
        }
        if self.step == Step::Window {
            let m = measured(&self.frames);
            return m.get(self.at).or(m.first()).copied();
        }
        self.frames.get(self.at).or(self.frames.first())
    }

    fn base_shown(&self) -> Option<&Frame> {
        if self.frames.is_empty() { None } else { self.frames.get(self.base_frame.min(self.frames.len() - 1)) }
    }

    /// The frames whose full resolution the loupe holds ready: the base frame from the roll's entry, its
    /// neighbours on the base step, none past it, so the loupe's first picture is already the sensor's.
    fn fulls_wanted(&self) -> Vec<String> {
        let Some(at) = self.base_shown().map(|_| self.base_frame.min(self.frames.len() - 1)) else { return Vec::new() };
        let mut wanted = match self.step.index().cmp(&Step::Base.index()) {
            std::cmp::Ordering::Greater => return Vec::new(),
            std::cmp::Ordering::Less => vec![at],
            std::cmp::Ordering::Equal => vec![at, at + 1, at.wrapping_sub(1)],
        };
        wanted.retain(|i| *i < self.frames.len());
        wanted.into_iter().map(|i| self.frames[i].path.clone()).collect()
    }

    fn ticked(&self) -> usize {
        self.frames.iter().filter(|f| f.ticked).count()
    }

    fn flagged(&self) -> usize {
        self.frames.iter().filter(|f| f.ticked && f.flagged).count()
    }

    fn post(&self, job: Job) {
        if let Some(s) = &self.serial {
            let _ = s.send(job);
        }
    }

    /// `resume(_:at:)`: the first draft found sets where the roll stands; each frame then takes its
    /// own place in it, a frame of no draft or of another's left out.
    fn resume(&mut self, draft: Option<&RollDraft>, index: usize) {
        let Some(path) = self.frames.get(index).map(|f| f.path.clone()) else { return };
        if let Some(d) = draft
            && self.draft_id.is_none()
        {
            self.draft_id = Some(d.id);
            self.step = Step::named(&d.step).unwrap_or(self.step);
            self.capture = Capture::from_raw(&d.capture).unwrap_or(self.capture);
            self.set_film(d.film);
            self.pass_order = d.pass_order.unwrap_or(self.pass_order);
            self.flat = d.flat_field.clone().unwrap_or_else(|| self.flat.clone());
            // Written back in this machine's spelling with the next draft.
            self.base = d.base.as_ref().and_then(|b| Some(BasePick { path: draft_base::found(&b.path, &self.frames)?, unit: (b.x, b.y), radius: b.radius }));
            self.window = d.window.unwrap_or(self.window);
            let draftless = std::mem::take(&mut self.draftless);
            for f in self.frames.iter_mut().filter(|f| draftless.contains(&f.path)) {
                f.ticked = false;
                f.flagged = false;
            }
            self.draftless = draftless;
        }
        let Some(id) = self.draft_id else {
            self.draftless.insert(path);
            return;
        };
        let mine = draft.is_some_and(|d| d.id == id);
        let f = &mut self.frames[index];
        f.ticked = mine && draft.is_some_and(|d| d.ticked);
        f.flagged = mine && draft.is_some_and(|d| d.flagged);
    }

    /// The draft every frame shares, its own ticked and flagged laid per frame; `None` until it may be written.
    fn drafts(&mut self) -> Option<Vec<(String, RollDraft)>> {
        if !self.shown || self.progress.is_some() || (self.step == Step::Film && self.draft_id.is_none()) || self.sources.is_empty() || self.read.len() < self.sources.len() {
            return None;
        }
        let id = *self.draft_id.get_or_insert_with(Id::new);
        let shared = RollDraft {
            id,
            step: self.step.name().into(),
            film: self.film,
            capture: self.capture.raw().into(),
            pass_order: Some(self.pass_order),
            flat_field: Some(self.flat.clone()),
            base: self.base.as_ref().map(|b| DraftBase { path: b.path.clone(), x: b.unit.0, y: b.unit.1, radius: b.radius }),
            window: Some(self.window),
            ticked: false,
            flagged: false,
        };
        Some(self.frames.iter().map(|f| (f.path.clone(), RollDraft { ticked: f.ticked, flagged: f.flagged, ..shared.clone() })).collect())
    }
}

/// The scan's pixel size, read off its container's header.
fn scan_size(path: &str) -> Option<(usize, usize)> {
    oni_decode::container::pixel_size(Path::new(path))
}

fn mosaic(path: &str) -> bool {
    oni_decode::decodes_via_mosaic(Path::new(path))
}

fn picture(b: &Bitmap) -> Image {
    Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&b.rgba, b.width as u32, b.height as u32))
}

/// The radius the base step's Picker Size is remembered at, `Pipette.radius` of the stored value;
/// the value a drag holds first, from memory, as every publish reads it.
fn base_radius(s: &State) -> f64 {
    let stored = s.prepare.picker.map(f64::from).or_else(|| s.preferences.value(roll::BASE_RADIUS_KEY).and_then(serde_json::Value::as_f64));
    pipette::radius(stored.unwrap_or(roll::BASE_RADIUS))
}

/// The sliders the panel holds, by their index in `Preparing.fractions`.
const AMOUNT: usize = 0;
const TEMPERATURE: usize = 1;
const TINT: usize = 2;
const PICKER: usize = 3;
const SCRUBBER: usize = 4;

fn flat_field(slider: usize) -> Field {
    match slider {
        AMOUNT => Field::FlatAmount,
        TEMPERATURE => Field::FlatTemperature,
        _ => Field::FlatTint,
    }
}
