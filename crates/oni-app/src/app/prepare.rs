//! Prepare the Roll (`.roll`): five steps over the roll's frames, a draft in each of their folders,
//! the measure off this thread with its progress and Stop, landed as one gallery step, then the editor.

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
use oni_settings::sidecar;
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
mod draft;
mod trace;
mod negative;
mod slides;
pub use model::*;

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
    /// The window the roll's first frame brought, before any draft or hand moved it.
    laid_window: Rect,
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
    /// A step taken before every frame was read, its draft written once the last one is.
    draft_waiting: bool,
    /// Bumped at each entry and landing, so a late post of a session gone lands nowhere.
    session: u64,
    previews: Option<Previews>,
    serial: Option<mpsc::Sender<Job>>,
    tiles: HashMap<String, Image>,
    card: Option<(String, Image)>,
    layers: card::Layers,
    /// The base's scan, its size, and its size at full resolution.
    scan: Option<(String, Image, (usize, usize), (usize, usize))>,
    /// The base frame whose scan cannot be read, and why.
    unreadable: Option<(String, String)>,
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
    /// The measure landed on one frame alone, until the editor leaves it.
    landed_alone: bool,
    pub require: bool,
    tutorial: Option<tutorial::Playing>,
    /// The steps' clips put away by Hide, until Show How or the next time the roll is prepared.
    tutorial_hidden: bool,
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
            window: oni_settings::geometry::proposal(Rect::UNIT, false),
            laid_window: oni_settings::geometry::proposal(Rect::UNIT, false),
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
            draft_waiting: false,
            session: 0,
            previews: None,
            serial: None,
            tiles: HashMap::new(),
            card: None,
            layers: card::Layers::default(),
            scan: None,
            unreadable: None,
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
            landed_alone: false,
            require: false,
            tutorial: None,
            tutorial_hidden: false,
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

    pub fn card_shown(&self) -> Option<&str> {
        self.layers.top()
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

    /// The frame the base step shows, by path.
    pub(in crate::app) fn base_path(&self) -> Option<String> {
        self.base_shown().map(|f| f.path.clone())
    }

    /// Every frame of the roll, by path, in its order.
    pub(in crate::app) fn frame_paths(&self) -> Vec<String> {
        self.frames.iter().map(|f| f.path.clone()).collect()
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

    /// A choice that leaving would drop: a base picked, a frame flagged or the window drawn.
    fn holds_choices(&self) -> bool {
        self.base.is_some() || self.flagged() > 0 || self.window != self.laid_window
    }

    fn post(&self, job: Job) {
        if let Some(s) = &self.serial {
            let _ = s.send(job);
        }
    }
}

/// A frame as Prepare the Roll shows and measures it: its whole scan, whatever its crop.
fn uncropped(s: &PipelineSettings) -> PipelineSettings {
    let mut out = s.clone();
    out.geometry.crop = Rect::UNIT;
    out.geometry.angle = 0.0;
    out
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
