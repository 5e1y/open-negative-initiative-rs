//! Prepare the Roll (`.roll`): five steps over the roll's frames, a draft in every sidecar, the
//! measure off this thread with its progress and Stop, landed as one gallery step, then the editor.

use super::{Ctl, State, frames, position, stored_at, with_shell};
use crate::fields::Field;
use crate::framing::Handle;
use crate::roll_previews::{Bitmap, Look, Out, Previews};
use crate::{App, MenuLine, Preparing, RollCell};
use oni_library::capture::CaptureExposure;
use oni_library::roll_membership;
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

/// `RollStep`, in the order the panel walks them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Step {
    #[default]
    Film,
    FlatField,
    Base,
    Flags,
    Window,
}

impl Step {
    pub const ALL: [Step; 5] = [Step::Film, Step::FlatField, Step::Base, Step::Flags, Step::Window];

    pub fn index(self) -> usize {
        Step::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    pub fn title(self) -> &'static str {
        match self {
            Step::Film => "Select the Film Type",
            Step::FlatField => "Set Up the Flat-Field",
            Step::Base => "Pick the Film Base",
            Step::Flags => "Flag the Frames Left Out",
            Step::Window => "Place the Measuring Frame",
        }
    }

    pub fn detail(self) -> &'static str {
        match self {
            Step::Film => "Choose the film and what you digitised it with. The wrong film type converts every frame of the roll wrong.",
            Step::FlatField => "Click the shot of the empty light panel, or pick one you loaded before. Skip it if your light is even; otherwise uneven light darkens or tints the corners of every frame.",
            Step::Base => "Choose a scan, then click on bare film: the border or the gap between two frames. A click on the picture instead shifts the blacks and colours of the whole roll.",
            Step::Flags => "Flag light leaks, the cut first frame and extreme shots: they are still converted, but left unflagged, one of them can wash out or tint the whole roll.",
            Step::Window => "Draw the frame inside the picture, clear of the holder and the film's edges, and use the slider to check it on every scan. An edge caught inside skews the whole roll.",
        }
    }

    /// The name a draft stores, the Swift case's, so a step added later moves no stored one.
    pub fn name(self) -> &'static str {
        match self {
            Step::Film => "film",
            Step::FlatField => "flatField",
            Step::Base => "base",
            Step::Flags => "flags",
            Step::Window => "window",
        }
    }

    pub fn named(name: &str) -> Option<Step> {
        Step::ALL.into_iter().find(|s| s.name() == name)
    }

    /// A scanner lights its film evenly: it needs no flat-field.
    pub fn skipped(self, capture: Capture) -> bool {
        self == Step::FlatField && capture == Capture::Scanner
    }
}

/// `RollPanel.neighbour`: one step either way past the skipped ones, held at either end.
pub fn neighbour(step: Step, way: i32, capture: Capture) -> Step {
    let at = |i: i32| usize::try_from(i).ok().and_then(|i| Step::ALL.get(i).copied());
    let mut next = at(step.index() as i32 + way).unwrap_or(step);
    while next.skipped(capture)
        && let Some(further) = at(next.index() as i32 + way)
    {
        next = further;
    }
    next
}

/// `RollPanel.stepFilled`: NEXT once the step holds its value, SKIP before.
pub fn filled(step: Step, flat: &FlatField, base: bool) -> bool {
    match step {
        Step::FlatField => !flat.is_neutral(),
        Step::Base => base,
        _ => true,
    }
}

/// `RollCapture`: what the roll was digitised with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Capture {
    Scanner,
    #[default]
    Camera,
    RgbPasses,
}

impl Capture {
    pub const ALL: [Capture; 3] = [Capture::Scanner, Capture::Camera, Capture::RgbPasses];

    pub fn raw(self) -> &'static str {
        match self {
            Capture::Scanner => "scanner",
            Capture::Camera => "camera",
            Capture::RgbPasses => "rgbPasses",
        }
    }

    pub fn from_raw(raw: &str) -> Option<Capture> {
        Capture::ALL.into_iter().find(|c| c.raw() == raw)
    }
}

/// `RollFrame`: a frame of the roll, its crop in its scan's fractions and its place in the measure.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub path: String,
    pub fingerprint: String,
    pub crop: Rect,
    /// Width over height of the scan after its quarter turns.
    pub aspect: Option<f64>,
    pub ticked: bool,
    /// Taking the roll's window but never measured, so it sets no end.
    pub flagged: bool,
    pub capture: Option<CaptureExposure>,
    pub passes: Option<RgbPasses>,
}

impl Frame {
    fn new(path: &str, fingerprint: &str) -> Frame {
        Frame { path: path.into(), fingerprint: fingerprint.into(), crop: Rect::UNIT, aspect: None, ticked: true, flagged: false, capture: None, passes: None }
    }
}

/// `RollFrame.measured`: the frames ticked and not flagged.
pub fn measured(frames: &[Frame]) -> Vec<&Frame> {
    frames.iter().filter(|f| f.ticked && !f.flagged).collect()
}

/// `RollFrame.aspect`: the scan's width over height after its quarter turns.
pub fn aspect(scan: Option<(usize, usize)>, geometry: &Geometry) -> Option<f64> {
    let (w, h) = scan?;
    let (tw, th) = geometry.oriented_size((w as f64, h as f64));
    (th > 0.0).then(|| tw / th)
}

/// `RollPicking.ownRoll`: the picked frame, those measured with its roll, and those never measured.
pub fn own_roll(picked: &str, folder: &[String], reference: impl Fn(&str) -> Option<Id>) -> Vec<String> {
    let own = reference(picked);
    folder.iter().filter(|p| *p == picked || reference(p).is_none_or(|r| Some(r) == own)).cloned().collect()
}

/// `RollPicking.flagging`: a click toggles the flag, and flagging brings an unticked frame back in.
pub fn flagging(frames: &mut [Frame], index: usize) {
    let Some(f) = frames.get_mut(index) else { return };
    f.flagged = !f.flagged;
    if f.flagged {
        f.ticked = true;
    }
}

/// `RollFrameView.fitted`: a box of `aspect` centred in `size`.
pub fn fitted(aspect: f64, size: (f64, f64)) -> (f64, f64, f64, f64) {
    if size.0 <= 0.0 || size.1 <= 0.0 || aspect <= 0.0 {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let w = size.0.min(size.1 * aspect);
    let h = w / aspect;
    ((size.0 - w) / 2.0, (size.1 - h) / 2.0, w, h)
}

/// `RollBaseSurface.fitted`: an image of `image` pixels scaled whole into `size`, centred.
pub fn fitted_image(image: (f64, f64), size: (f64, f64)) -> (f64, f64, f64, f64) {
    if image.0 <= 0.0 || image.1 <= 0.0 || size.0 <= 0.0 || size.1 <= 0.0 {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let scale = (size.0 / image.0).min(size.1 / image.1);
    let (w, h) = (image.0 * scale, image.1 * scale);
    ((size.0 - w) / 2.0, (size.1 - h) / 2.0, w, h)
}

/// The key's title on the window step.
pub fn measure_title(n: usize) -> String {
    if n == 1 { "MEASURE 1 FRAME".into() } else { format!("MEASURE {n} FRAMES") }
}

pub fn left_out_text(n: usize) -> String {
    match n {
        0 => String::new(),
        1 => "1 frame of this folder, measured on another roll, is left out.".into(),
        n => format!("{n} frames of this folder, measured on another roll, are left out."),
    }
}

/// The roll's most common capture and the frames read apart from it.
pub fn apart(frames: &[Frame]) -> (Option<CaptureExposure>, HashSet<String>) {
    let captures: Vec<Option<CaptureExposure>> = frames.iter().map(|f| f.capture).collect();
    let Some(common) = CaptureExposure::common(&captures) else { return (None, HashSet::new()) };
    let apart = frames.iter().filter(|f| f.capture.is_some_and(|c| c != common)).map(|f| f.path.clone()).collect();
    (Some(common), apart)
}

/// `DSWarning`'s text when the roll mixes captures, empty when it does not.
pub fn differ_text(frames: &[Frame]) -> String {
    let (common, apart) = apart(frames);
    let Some(common) = common.filter(|_| !apart.is_empty()) else { return String::new() };
    format!(
        "{} of {} frames, marked, were digitised with other settings than the roll's {}. Their exposure is evened out to it; a different aperture also changes the vignetting the flat-field measured, so re-shoot them in manual for the cleanest roll.",
        apart.len(),
        frames.len(),
        common.label()
    )
}

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

/// The roll being prepared: `mode == .roll` and every `roll…` value of the scene.
pub(crate) struct Roll {
    pub shown: bool,
    back_to_gallery: bool,
    frames: Vec<Frame>,
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
    scan: Option<(String, Image, (usize, usize))>,
    full: Option<(String, Image, (usize, usize))>,
    pointer: Option<(f64, f64)>,
    asked_scan: Option<String>,
    shown_path: Option<String>,
    window_gesture: Option<(Handle, Rect, (f64, f64))>,
    slide: Option<(usize, f32)>,
    last_press: Option<(usize, Instant)>,
    asked: HashSet<String>,
    /// `work.openedAmong`: the selection a load narrowed, when it held the frame opened.
    pub opened_among: HashSet<String>,
    pub require: bool,
}

impl Default for Roll {
    fn default() -> Self {
        Roll {
            shown: false,
            back_to_gallery: true,
            frames: Vec::new(),
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
            full: None,
            pointer: None,
            asked_scan: None,
            shown_path: None,
            window_gesture: None,
            slide: None,
            last_press: None,
            asked: HashSet::new(),
            opened_among: HashSet::new(),
            require: false,
        }
    }
}

impl Roll {
    pub(super) fn new(require: bool) -> Roll {
        Roll { require, ..Roll::default() }
    }

    /// Every frame's sidecar read, which the bench waits for.
    pub fn read_whole(&self) -> bool {
        !self.frames.is_empty() && self.read.len() >= self.frames.len()
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
            self.film = d.film;
            self.capture = Capture::from_raw(&d.capture).unwrap_or(self.capture);
            self.pass_order = d.pass_order.unwrap_or(self.pass_order);
            self.flat = d.flat_field.clone().unwrap_or_else(|| self.flat.clone());
            self.base = d.base.as_ref().map(|b| BasePick { path: b.path.clone(), unit: (b.x, b.y), radius: b.radius });
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
        if !self.shown || self.progress.is_some() || (self.step == Step::Film && self.draft_id.is_none()) || self.frames.is_empty() || self.read.len() < self.frames.len() {
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

/// The radius the base step's Picker Size is remembered at, `Pipette.radius` of the stored value.
fn base_radius(s: &State) -> f64 {
    let stored = crate::preferences::Preferences::load(&s.support).value(roll::BASE_RADIUS_KEY).and_then(serde_json::Value::as_f64);
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

impl Ctl {
    pub(super) fn wire_prepare(me: &Rc<Ctl>, window: &crate::MainWindow) {
        let (tx, rx) = mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("prepare the roll".into())
            .spawn(move || {
                for job in rx {
                    job();
                }
            })
            .expect("prepare thread");
        {
            let mut s = me.state.borrow_mut();
            s.prepare.serial = Some(tx);
            s.prepare.previews = Some(Previews::start(|session, out| with_shell(move |c| c.preview_landed(session, out))));
        }
        let p = window.global::<Preparing>();
        let c = me.clone();
        p.on_step_picked(move |i| c.step_to(Step::ALL.get(usize::try_from(i).unwrap_or(0)).copied().unwrap_or_default()));
        let c = me.clone();
        p.on_film_picked(move |i| {
            if let Some(m) = usize::try_from(i).ok().and_then(|i| ConversionMode::ALL.get(i).copied()) {
                c.state.borrow_mut().prepare.film = m;
                c.publish_prepare();
            }
        });
        let c = me.clone();
        p.on_capture_picked(move |i| {
            if let Some(capture) = usize::try_from(i).ok().and_then(|i| Capture::ALL.get(i).copied()) {
                c.state.borrow_mut().prepare.capture = capture;
                c.publish_prepare();
            }
        });
        let c = me.clone();
        p.on_flat_toggled(move |on| {
            {
                let mut s = c.state.borrow_mut();
                s.prepare.flat = super::roll_tab::toggled(&s.prepare.flat, on);
            }
            c.publish_prepare();
        });
        let c = me.clone();
        p.on_mask_chosen(move |i| c.roll_mask_chosen(usize::try_from(i).ok()));
        let c = me.clone();
        p.on_press(move |i, _| c.slider_press(usize::try_from(i).unwrap_or(usize::MAX)));
        let c = me.clone();
        p.on_drag(move |i, t, span, fine| c.slider_drag(usize::try_from(i).unwrap_or(usize::MAX), t, span, fine));
        let c = me.clone();
        p.on_release(move |_| c.state.borrow_mut().prepare.slide = None);
        let c = me.clone();
        p.on_clear_base(move || {
            c.state.borrow_mut().prepare.base = None;
            c.publish_prepare();
        });
        let c = me.clone();
        p.on_clear_flags(move || {
            for f in &mut c.state.borrow_mut().prepare.frames {
                f.flagged = false;
            }
            c.publish_prepare();
        });
        let c = me.clone();
        p.on_previous(move || c.walk_window(-1));
        let c = me.clone();
        p.on_next(move || c.walk_window(1));
        let c = me.clone();
        p.on_cell_clicked(move |i| c.cell_clicked(usize::try_from(i).unwrap_or(usize::MAX)));
        let c = me.clone();
        p.on_sheet_clicked(move |i| {
            flagging(&mut c.state.borrow_mut().prepare.frames, usize::try_from(i).unwrap_or(usize::MAX));
            c.publish_prepare();
        });
        let c = me.clone();
        p.on_cancel(move || c.roll_cancel());
        let c = me.clone();
        p.on_back(move || {
            let (step, capture) = {
                let r = &c.state.borrow().prepare;
                (r.step, r.capture)
            };
            c.step_to(neighbour(step, -1, capture));
        });
        let c = me.clone();
        p.on_advance(move || c.roll_advance());
        let c = me.clone();
        p.on_card_resized(move || c.publish_prepare());
        let c = me.clone();
        p.on_window_grabbed(move |h| c.window_grabbed(h));
        let c = me.clone();
        p.on_window_dragged(move |_, dx, dy| c.window_dragged(f64::from(dx), f64::from(dy)));
        let c = me.clone();
        p.on_window_released(move || c.state.borrow_mut().prepare.window_gesture = None);
        let c = me.clone();
        p.on_base_previous(move || c.walk_base(-1));
        let c = me.clone();
        p.on_base_next(move || c.walk_base(1));
        let c = me.clone();
        p.on_base_resized(move || c.publish_prepare());
        let c = me.clone();
        p.on_base_hover(move |x, y, on| {
            c.state.borrow_mut().prepare.pointer = on.then_some((f64::from(x), f64::from(y)));
            c.publish_base();
        });
        let c = me.clone();
        p.on_base_clicked(move |x, y| c.base_clicked(f64::from(x), f64::from(y)));
        let c = me.clone();
        window.global::<crate::PanelState>().on_prepare_roll(move || c.prepare_roll());
    }

    // MARK: - Entering and leaving

    /// `prepareRoll(propagationTargets)`: one frame brings its folder's own roll, several come as they are.
    pub(super) fn prepare_roll(self: &Rc<Self>) {
        let targets = self.targets();
        self.roll_of(targets, |c, paths, left| c.enter_roll(&paths, left));
    }

    /// `roll(of:)`: the frame's gallery group read off this thread, kept to its own roll.
    fn roll_of(self: &Rc<Self>, targets: Vec<String>, then: impl FnOnce(&Rc<Ctl>, Vec<String>, usize) + Send + 'static) {
        let folder: Option<Vec<String>> = {
            let s = self.state.borrow();
            match targets.as_slice() {
                [picked] => s.org.bands.iter().find(|b| b.entries.iter().any(|e| &e.path == picked)).map(|b| b.entries.iter().map(|e| e.path.clone()).collect()),
                _ => None,
            }
        };
        std::thread::spawn(move || {
            let (paths, left) = match folder {
                Some(folder) => {
                    let own = own_roll(&targets[0], &folder, roll_membership::reference);
                    let left = folder.len() - own.len();
                    (own, left)
                }
                None => (targets, 0),
            };
            with_shell(move |c| then(c, paths, left));
        });
    }

    /// `enterRollReference`: every frame ticked, the film step, then each sidecar read off this thread.
    fn enter_roll(self: &Rc<Self>, paths: &[String], left_out: usize) {
        let Some(w) = self.window() else { return };
        let wanted: HashSet<&String> = paths.iter().filter(|p| !roll_membership::is_sheet(p)).collect();
        let (ordered, session) = {
            let mut s = self.state.borrow_mut();
            let ordered: Vec<(String, String)> = s.rows.iter().filter(|e| wanted.contains(&e.path)).map(|e| (e.path.clone(), e.fingerprint.clone())).collect();
            if ordered.is_empty() || s.prepare.progress.is_some() {
                return;
            }
            let gallery = w.global::<App>().get_gallery();
            s.shelf.hovered = None;
            let r = &mut s.prepare;
            if !r.shown {
                r.back_to_gallery = gallery;
            }
            r.frames = ordered.iter().map(|(p, f)| Frame::new(p, f)).collect();
            r.left_out = left_out;
            r.read.clear();
            r.step = Step::Film;
            r.at = 0;
            r.flat_pick = None;
            r.session += 1;
            r.tiles.clear();
            r.card = None;
            r.scan = None;
            r.full = None;
            r.asked_scan = None;
            r.shown_path = None;
            r.stop.store(false, Ordering::Relaxed);
            r.base = None;
            r.base_frame = 0;
            r.draft_id = None;
            r.draftless.clear();
            r.shown = true;
            if let Some(p) = &r.previews {
                p.reset(r.session);
            }
            (ordered, r.session)
        };
        let paths: Vec<String> = ordered.into_iter().map(|(p, _)| p).collect();
        let reading = paths.clone();
        std::thread::spawn(move || {
            let captures: Vec<Option<CaptureExposure>> = reading.iter().map(|p| CaptureExposure::read(Path::new(p))).collect();
            with_shell(move |c| {
                {
                    let mut s = c.state.borrow_mut();
                    if s.prepare.session != session {
                        return;
                    }
                    for (path, capture) in reading.iter().zip(captures) {
                        if let Some(f) = s.prepare.frames.iter_mut().find(|f| &f.path == path) {
                            f.capture = capture;
                        }
                    }
                }
                c.publish_prepare();
            });
        });
        // Framings land frame by frame, so nothing waits for the slowest file before the first scan shows.
        std::thread::spawn(move || {
            for (index, path) in paths.into_iter().enumerate() {
                let loaded = sidecar::read(Path::new(&path), mosaic(&path), false).ok();
                let read = loaded.as_ref().map(|l| l.settings.clone()).unwrap_or_default();
                let draft = loaded.and_then(|l| l.roll_draft);
                let aspect = aspect(scan_size(&path), &read.geometry);
                with_shell(move |c| c.land_frame(session, &path, read, draft, index == 0, aspect));
            }
        });
        self.publish_menus();
        self.publish_bar();
        self.publish_prepare();
        w.invoke_refocus();
    }

    /// `landRollFrame`: the frame's framing, its preview asked, the roll's values off the first.
    fn land_frame(self: &Rc<Self>, session: u64, path: &str, read: PipelineSettings, draft: Option<RollDraft>, first: bool, aspect: Option<f64>) {
        {
            let mut s = self.state.borrow_mut();
            let r = &mut s.prepare;
            if !r.shown || r.session != session {
                return;
            }
            r.read.insert(path.to_string(), read.clone());
            let Some(index) = r.frames.iter().position(|f| f.path == path) else { return };
            let f = &mut r.frames[index];
            if read.rgb_passes.is_some() && f.passes.is_none() {
                f.passes = read.rgb_passes.clone();
            }
            f.crop = read.geometry.crop;
            f.aspect = aspect;
            if let Some(p) = &r.previews {
                p.add(path, &read);
            }
            if first {
                if let Some(a) = aspect {
                    r.aspect = a;
                }
                r.window = read.geometry.measure_window.unwrap_or_else(|| oni_measure::window::proposal(&read.geometry.crop, false));
                r.flat = read.flat_field.clone();
                r.film = read.mode;
                r.capture = if read.rgb_passes.is_some() {
                    Capture::RgbPasses
                } else if read.decoded_from_mosaic {
                    Capture::Camera
                } else {
                    Capture::Scanner
                };
            }
            r.resume(draft.as_ref(), index);
        }
        self.publish_prepare();
    }

    /// `leaveRollReference`: refused while measuring; the draft saved, back where the hand came from.
    pub(super) fn leave_roll(self: &Rc<Self>) {
        {
            let s = self.state.borrow();
            if !s.prepare.shown || s.prepare.progress.is_some() {
                return;
            }
        }
        self.save_draft();
        let back = {
            let mut s = self.state.borrow_mut();
            let r = &mut s.prepare;
            r.shown = false;
            r.session += 1;
            r.tiles.clear();
            r.card = None;
            r.scan = None;
            r.full = None;
            if let Some(p) = &r.previews {
                p.reset(r.session);
            }
            r.back_to_gallery
        };
        if let Some(w) = self.window() {
            w.global::<App>().set_gallery(back);
        }
        self.publish_prepare();
        self.publish_tiles();
        self.publish_bar();
        self.request_render();
    }

    /// `takeOff`: a frame opened from the sidebar leaves the roll first, never while it measures.
    pub(super) fn take_off_roll(self: &Rc<Self>) -> bool {
        let (shown, measuring) = {
            let r = &self.state.borrow().prepare;
            (r.shown, r.progress.is_some())
        };
        if shown && measuring {
            return false;
        }
        if shown {
            self.leave_roll();
        }
        true
    }

    /// `requireRoll`: a frame never measured, opened in the editor, goes to Prepare the Roll on its
    /// selection or its folder; Cancel then leaves for the gallery.
    pub(super) fn require_roll(self: &Rc<Self>, path: &str) {
        let picked: Vec<String> = {
            let mut s = self.state.borrow_mut();
            let writable = s.store.editor().is_some_and(|e| e.writable());
            let settings = s.store.settings();
            if !s.prepare.require || !writable || settings.roll_reference.is_some() || settings.sheet.is_some() {
                return;
            }
            let among = std::mem::take(&mut s.prepare.opened_among);
            if among.contains(path) { s.order.iter().filter(|p| among.contains(*p)).cloned().collect() } else { vec![path.to_string()] }
        };
        let path = path.to_string();
        self.roll_of(picked, move |c, paths, left| {
            let still = {
                let s = c.state.borrow();
                let gallery = c.window().is_none_or(|w| w.global::<App>().get_gallery());
                s.open.as_deref() == Some(path.as_str()) && s.store.settings().roll_reference.is_none() && !gallery && !s.prepare.shown
            };
            if !still {
                return;
            }
            c.enter_roll(&paths, left);
            if c.state.borrow().prepare.shown {
                c.state.borrow_mut().prepare.back_to_gallery = true;
            }
        });
    }

    // MARK: - The steps

    fn step_to(self: &Rc<Self>, step: Step) {
        let changed = {
            let r = &mut self.state.borrow_mut().prepare;
            if !r.shown || r.progress.is_some() || step.skipped(r.capture) {
                return;
            }
            let changed = r.step != step;
            r.step = step;
            changed
        };
        if changed {
            self.save_draft();
        }
        self.publish_prepare();
    }

    fn roll_advance(self: &Rc<Self>) {
        let (step, capture) = {
            let r = &self.state.borrow().prepare;
            (r.step, r.capture)
        };
        if step == Step::Window {
            return self.measure_roll();
        }
        self.step_to(neighbour(step, 1, capture));
    }

    /// `onCancel`: Stop while measuring, else the roll left.
    pub(super) fn roll_cancel(self: &Rc<Self>) {
        let measuring = {
            let r = &self.state.borrow().prepare;
            if r.progress.is_some() {
                r.stop.store(true, Ordering::Relaxed);
            }
            r.progress.is_some()
        };
        if !measuring {
            self.leave_roll();
        }
    }

    /// Escape and, on the window step, the arrows; any other key goes on to the menus.
    pub(super) fn prepare_key(self: &Rc<Self>, text: &str, command: bool, shift: bool, bare: bool) -> bool {
        if text == "\u{1b}" {
            self.roll_cancel();
            return true;
        }
        let (step, measuring) = {
            let r = &self.state.borrow().prepare;
            (r.step, r.progress.is_some())
        };
        if step != Step::Window || measuring || command || !(bare || shift) {
            return false;
        }
        match text {
            "\u{F702}" => self.walk_window(-1),
            "\u{F703}" => self.walk_window(1),
            _ => return false,
        }
        true
    }

    fn walk_window(self: &Rc<Self>, by: i32) {
        {
            let r = &mut self.state.borrow_mut().prepare;
            let last = measured(&r.frames).len().saturating_sub(1) as i32;
            r.at = (r.at as i32 + by).clamp(0, last.max(0)) as usize;
        }
        self.publish_prepare();
    }

    fn walk_base(self: &Rc<Self>, by: i32) {
        {
            let r = &mut self.state.borrow_mut().prepare;
            let last = r.frames.len().saturating_sub(1) as i32;
            r.base_frame = (r.base_frame as i32 + by).clamp(0, last.max(0)) as usize;
            r.pointer = None;
        }
        self.publish_prepare();
    }

    /// `RollPanel.click`: the flat-field's pick, or the scan the base step shows.
    fn cell_clicked(self: &Rc<Self>, i: usize) {
        {
            let r = &mut self.state.borrow_mut().prepare;
            let Some(path) = r.frames.get(i).map(|f| f.path.clone()) else { return };
            match r.step {
                Step::FlatField => r.flat_pick = Some(path),
                Step::Base => {
                    r.base_frame = i;
                    r.pointer = None;
                }
                _ => return,
            }
        }
        self.publish_prepare();
    }

    fn roll_mask_chosen(self: &Rc<Self>, i: Option<usize>) {
        {
            let mut s = self.state.borrow_mut();
            let options = super::roll_tab::mask_options(&s.prepare.flat, &s.masks);
            let Some((line, _)) = i.and_then(|i| options.get(i)).cloned() else { return };
            s.prepare.flat = super::roll_tab::chose(&s.prepare.flat, line);
        }
        self.publish_prepare();
    }

    // MARK: - The sliders

    fn slider_value(r: &Roll, radius: f64, i: usize) -> f32 {
        match i {
            AMOUNT => r.flat.amount,
            TEMPERATURE => r.flat.temperature,
            TINT => r.flat.tint,
            PICKER => radius as f32,
            _ => r.at as f32,
        }
    }

    /// Where a slider's knob sits: the flat-field's on their own tracks, the others linear.
    fn slider_fraction(r: &Roll, radius: f64, i: usize) -> f32 {
        let v = Self::slider_value(r, radius, i);
        match i {
            AMOUNT | TEMPERATURE | TINT => position(flat_field(i), v),
            PICKER => ((f64::from(v) - pipette::RADIUS_RANGE.0) / (pipette::RADIUS_RANGE.1 - pipette::RADIUS_RANGE.0)) as f32,
            _ => {
                let last = measured(&r.frames).len().saturating_sub(1).max(1);
                v / last as f32
            }
        }
    }

    /// `DSSlider`'s press: a second within the double-click interval resets it to its base.
    fn slider_press(self: &Rc<Self>, i: usize) -> bool {
        let now = Instant::now();
        let double = {
            let mut s = self.state.borrow_mut();
            let radius = base_radius(&s);
            let r = &mut s.prepare;
            let double = r.last_press.is_some_and(|(j, at)| j == i && now.duration_since(at) <= crate::system::double_click());
            r.last_press = Some((i, now));
            r.slide = (!double).then(|| (i, Self::slider_fraction(r, radius, i)));
            double
        };
        if double {
            let base = match i {
                AMOUNT => FlatField::default().amount,
                PICKER => roll::BASE_RADIUS as f32,
                _ => 0.0,
            };
            self.slider_set(i, base);
        }
        double
    }

    fn slider_drag(self: &Rc<Self>, i: usize, translation: f32, span: f32, fine: bool) {
        let Some((held, start)) = self.state.borrow().prepare.slide else { return };
        if held != i {
            return;
        }
        let at = crate::track::clamped(crate::track::asked(start, translation, span, fine));
        let value = match i {
            AMOUNT | TEMPERATURE | TINT => stored_at(flat_field(i), at),
            PICKER => (pipette::RADIUS_RANGE.0 + f64::from(at) * (pipette::RADIUS_RANGE.1 - pipette::RADIUS_RANGE.0)) as f32,
            _ => {
                let last = measured(&self.state.borrow().prepare.frames).len().saturating_sub(1).max(1);
                at * last as f32
            }
        };
        self.slider_set(i, value);
    }

    fn slider_set(self: &Rc<Self>, i: usize, value: f32) {
        {
            let mut s = self.state.borrow_mut();
            match i {
                AMOUNT => s.prepare.flat.amount = value,
                TEMPERATURE => s.prepare.flat.temperature = value,
                TINT => s.prepare.flat.tint = value,
                PICKER => {
                    let mut prefs = crate::preferences::Preferences::load(&s.support);
                    let _ = prefs.set(roll::BASE_RADIUS_KEY, serde_json::Value::from(f64::from(value)));
                }
                SCRUBBER => {
                    let r = &mut s.prepare;
                    let last = measured(&r.frames).len().saturating_sub(1);
                    r.at = (value.round().max(0.0) as usize).min(last);
                }
                _ => {}
            }
        }
        self.publish_prepare();
    }

    // MARK: - The card and the base's scan

    fn card_box(&self) -> Option<(f64, f64, f64, f64)> {
        let w = self.window()?;
        let p = w.global::<Preparing>();
        let r = &self.state.borrow().prepare;
        let aspect = r.shown_frame().and_then(|f| f.aspect).unwrap_or(r.aspect);
        Some(fitted(aspect, (f64::from(p.get_card_width()), f64::from(p.get_card_height()))))
    }

    fn window_grabbed(self: &Rc<Self>, handle: i32) {
        let (Some(handle), Some(b)) = (Handle::from_index(handle), self.card_box()) else { return };
        let r = &mut self.state.borrow_mut().prepare;
        r.window_gesture = Some((handle, r.window, (b.2, b.3)));
    }

    /// `MeasureWindowOverlay.bounded`: the window kept inside the whole scan, never under 5 %.
    fn window_dragged(self: &Rc<Self>, dx: f64, dy: f64) {
        {
            let r = &mut self.state.borrow_mut().prepare;
            let Some((handle, from, size)) = r.window_gesture else { return };
            r.window = frames::bounded(frames::apply(handle, from, size, (dx, dy)));
        }
        self.publish_card();
    }

    /// The scan's rectangle in the base area, and the scan's own size in pixels.
    fn scan_rect(&self) -> Option<((f64, f64, f64, f64), (usize, usize))> {
        let w = self.window()?;
        let p = w.global::<Preparing>();
        let r = &self.state.borrow().prepare;
        let path = r.base_shown()?.path.clone();
        let (_, _, size) = r.scan.as_ref().filter(|(p, _, _)| *p == path)?;
        let rect = fitted_image((size.0 as f64, size.1 as f64), (f64::from(p.get_base_width()), f64::from(p.get_base_height())));
        Some((rect, *size))
    }

    /// A click inside the scan picks the bare film there, then the roll goes on to its flags.
    fn base_clicked(self: &Rc<Self>, x: f64, y: f64) {
        let Some((rect, _)) = self.scan_rect() else { return };
        if x < rect.0 || y < rect.1 || x > rect.0 + rect.2 || y > rect.1 + rect.3 {
            return;
        }
        {
            let mut s = self.state.borrow_mut();
            let radius = base_radius(&s);
            let r = &mut s.prepare;
            let Some(path) = r.base_shown().map(|f| f.path.clone()) else { return };
            r.base = Some(BasePick { path, unit: ((x - rect.0) / rect.2, (y - rect.1) / rect.3), radius });
        }
        self.step_to(Step::Flags);
    }

    // MARK: - The measure

    /// `measureRoll`: every ticked frame read off this thread after the drafts' writes, the flagged
    /// ones riding without a vote; the base read, the roll combined, every frame written onto it.
    fn measure_roll(self: &Rc<Self>) {
        let (job, session) = {
            let mut s = self.state.borrow_mut();
            let open = s.open.clone();
            let live = s.store.settings().clone();
            let r = &mut s.prepare;
            let picked: Vec<Frame> = r.frames.iter().filter(|f| f.ticked).cloned().collect();
            if !picked.iter().any(|f| !f.flagged) || r.progress.is_some() {
                return;
            }
            r.stop.store(false, Ordering::Relaxed);
            r.progress = Some((0, picked.len()));
            let (window, mask, pick, film, stop, session) = (r.window, r.mask(), r.base.clone(), r.film, r.stop.clone(), r.session);
            let job: Job = Box::new(move || {
                let paths: Vec<String> = picked.iter().map(|f| f.path.clone()).collect();
                let asked = paths.len();
                let common = CaptureExposure::common(&paths.iter().map(|p| CaptureExposure::read(Path::new(p))).collect::<Vec<_>>());
                let stops = |path: &str| match (common, CaptureExposure::read(Path::new(path))) {
                    (Some(c), Some(own)) => own.stops() - c.stops(),
                    _ => 0.0,
                };
                let m = crate::measuring::measurer();
                let mut jobs: Vec<roll::Job> = Vec::new();
                let mut frames = Vec::new();
                for f in &picked {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    let before = if Some(&f.path) == open.as_ref() {
                        live.clone()
                    } else {
                        match sidecar::read(Path::new(&f.path), mosaic(&f.path), true) {
                            Ok(l) if l.writable => l.settings,
                            _ => continue,
                        }
                    };
                    let start = roll::start(&before, film, window, mask.as_ref(), stops(&f.path));
                    let job = roll::Job { path: f.path.clone(), before, start, flagged: f.flagged };
                    if f.flagged {
                        jobs.push(job);
                    } else if let Some(frame) = m.and_then(|m| m.with(&f.path, &job.start, |r, copy| key::roll_measured(r, copy, &f.path, &job.start))) {
                        frames.push(frame);
                        jobs.push(job);
                    }
                    with_shell(move |c| c.measure_progressed(session));
                }
                let stopped = stop.load(Ordering::Relaxed);
                let base = pick.filter(|_| !stopped).and_then(|p| {
                    let bare = if Some(&p.path) == open.as_ref() {
                        live.clone()
                    } else {
                        sidecar::read(Path::new(&p.path), mosaic(&p.path), true).map(|l| l.settings).unwrap_or_default()
                    };
                    let unit = Point::new(p.unit.0, p.unit.1);
                    m?.with(&p.path, &bare, |r, copy| key::base(r, copy, &bare, film, mask.as_ref(), stops(&p.path), unit, p.radius))
                });
                let reference = if stopped { None } else { roll::combined(&frames, film, base) };
                let written: Option<Written> =
                    reference.map(|reference| jobs.iter().map(|j| (j.path.clone(), j.before.clone(), oni_core::balance::written(&reference, &j.start))).collect());
                with_shell(move |c| c.land_measure(session, written, stopped, asked));
            });
            (job, session)
        };
        let _ = session;
        self.state.borrow().prepare.post(job);
        self.publish_prepare();
        self.publish_bar();
    }

    fn measure_progressed(self: &Rc<Self>, session: u64) {
        {
            let r = &mut self.state.borrow_mut().prepare;
            if r.session != session {
                return;
            }
            if let Some(p) = r.progress.as_mut() {
                p.0 += 1;
            }
        }
        self.publish_prepare();
    }

    /// `landRoll`: nothing measured keeps the roll open and says why; else every frame written as
    /// one gallery step, the drafts dropped, and the editor on the first frame unless the open one landed.
    fn land_measure(self: &Rc<Self>, session: u64, written: Option<Written>, stopped: bool, asked: usize) {
        let prepared: Vec<String> = {
            let mut s = self.state.borrow_mut();
            let r = &mut s.prepare;
            if r.session != session {
                return;
            }
            r.progress = None;
            r.session += 1;
            r.tiles.clear();
            r.card = None;
            if let Some(p) = &r.previews {
                p.reset(r.session);
            }
            r.frames.iter().map(|f| f.path.clone()).collect()
        };
        let Some(written) = written.filter(|w| !w.is_empty()) else {
            if let Some(store) = self.state.borrow_mut().store.editor() {
                store.forget_measured();
            }
            if !stopped {
                self.advise("Something Went Wrong", "No frame of this roll could be measured: the files could not be read, or the measuring frame holds nothing to balance.");
            }
            self.publish_prepare();
            self.publish_bar();
            return;
        };
        let (failure, landed) = {
            let mut s = self.state.borrow_mut();
            let Some(store) = s.store.editor() else { return };
            let landed = store.land_roll(&written, &prepared);
            let failure = store.failure.take();
            s.last_change = Some(Instant::now());
            s.prepare.draft_id = None;
            s.prepare.shown = false;
            (failure, landed)
        };
        let support = self.state.borrow().support.clone();
        oni_library::journal::Journal::in_support(&support).note(oni_library::journal::Kind::Measured, None, 0, landed as u32, None, oni_settings::Date::now());
        let message = failure.or_else(|| (written.len() < asked).then(|| format!("{} of {asked} frames could not be measured and were left as they were.", asked - written.len())));
        let open = self.state.borrow().open.clone();
        if let Some(w) = self.window() {
            w.global::<App>().set_gallery(false);
        }
        let touched: Vec<String> = written.iter().map(|(p, _, _)| p.clone()).collect();
        self.landed(&touched);
        if open.is_none() || !written.iter().any(|(p, _, _)| Some(p) == open.as_ref()) {
            let first = written[0].0.clone();
            self.state.borrow_mut().designate(&first);
            self.open_photo(&first);
            self.publish_tiles();
        }
        if let Some(m) = message {
            self.advise("Something Went Wrong", &m);
        }
        self.publish_prepare();
        self.publish_bar();
    }

    /// `saveRollDraft`: each frame's draft written on the serial thread, after the writes before it.
    fn save_draft(&self) {
        let job = {
            let r = &mut self.state.borrow_mut().prepare;
            let Some(drafts) = r.drafts() else { return };
            let job: Job = Box::new(move || {
                for (path, draft) in drafts {
                    let _ = sidecar::set_roll_draft(Some(draft), Path::new(&path), mosaic(&path));
                }
            });
            job
        };
        self.state.borrow().prepare.post(job);
    }

    // MARK: - Previews

    fn preview_landed(self: &Rc<Self>, session: u64, out: Out) {
        {
            let r = &mut self.state.borrow_mut().prepare;
            if r.session != session || !r.shown {
                return;
            }
            match out {
                Out::Tile(path, b) => {
                    r.tiles.insert(path, picture(&b));
                }
                Out::Card(path, b) => r.card = Some((path, picture(&b))),
                Out::Scan(path, b) => {
                    let size = (b.width, b.height);
                    r.scan = Some((path, picture(&b), size));
                }
                Out::Full(path, b) => {
                    let size = (b.width, b.height);
                    r.full = Some((path, picture(&b), size));
                }
            }
        }
        self.publish_prepare();
    }

    /// The thumbnails of the roll's frames asked of the folder, as the grid asks its visible cells.
    fn ask_thumbnails(s: &mut State) {
        let wanted: Vec<String> = s
            .prepare
            .frames
            .iter()
            .map(|f| f.fingerprint.clone())
            .filter(|f| !f.starts_with(crate::thumbnails::PENDING) && !s.thumbs.contains(f) && !s.asked.contains(f) && !s.prepare.asked.contains(f))
            .collect();
        for f in wanted {
            s.asked.insert(f.clone());
            s.prepare.asked.insert(f.clone());
            let _ = s.disk.send(f);
        }
    }

    // MARK: - Publishing

    /// Every value the roll's panel, card and step keys show, read before any is set.
    pub(super) fn publish_prepare(&self) {
        let Some(w) = self.window() else { return };
        let p = w.global::<Preparing>();
        let shown = self.state.borrow().prepare.shown;
        p.set_shown(shown);
        if !shown {
            p.set_measuring(false);
            return;
        }
        let mut s = self.state.borrow_mut();
        Self::ask_thumbnails(&mut s);
        let radius = base_radius(&s);
        let masks = s.masks.clone();
        let settled = s.prepare.frames.iter().filter(|f| s.thumbs.contains(&f.fingerprint) || s.failed.contains(&f.fingerprint)).count();
        let frames = s.prepare.frames.clone();
        let mut pictures = Vec::with_capacity(frames.len());
        for f in &frames {
            let tile = s.prepare.tiles.get(&f.path).cloned();
            let picture = match tile {
                Some(t) => t,
                None => s.thumbs.get(&f.fingerprint).unwrap_or_default(),
            };
            pictures.push(picture);
        }
        let shown_frame = s.prepare.shown_frame().cloned();
        let card = shown_frame.as_ref().map(|f| match &s.prepare.card {
            Some((path, image)) if *path == f.path => image.clone(),
            _ => s.prepare.tiles.get(&f.path).cloned().unwrap_or_default(),
        });
        let card = match (card, &shown_frame) {
            (Some(c), Some(f)) if c.size().width == 0 => s.thumbs.get(&f.fingerprint).unwrap_or_default(),
            (c, _) => c.unwrap_or_default(),
        };
        let r = &s.prepare;
        let (_, apart) = apart(&frames);
        let ticked = r.ticked();
        let flagged = r.flagged();
        let measuring = r.progress;
        let m = measured(&frames);
        let last = m.len().saturating_sub(1);
        let step = r.step;
        let status = match measuring {
            Some((done, total)) => format!("Measuring the roll · {done} of {total}"),
            None if settled < frames.len() => format!("Loading the roll's thumbnails · {settled} of {}", frames.len()),
            None => String::new(),
        };
        let cells: Vec<RollCell> = frames
            .iter()
            .zip(pictures)
            .enumerate()
            .map(|(i, (f, picture))| RollCell {
                picture,
                marked: match step {
                    Step::FlatField => r.flat_pick.as_deref() == Some(f.path.as_str()),
                    Step::Base => r.base_frame == i,
                    _ => f.ticked,
                },
                flagged: f.flagged,
                apart: apart.contains(&f.path),
                ticked: f.ticked,
                rgb: f.passes.is_some(),
            })
            .collect();
        let options = super::roll_tab::mask_options(&r.flat, &masks);
        let current = super::roll_tab::chosen(&r.flat);
        let mask_title = options.iter().find(|(l, _)| *l == current).map_or("None selected".to_string(), |(_, t)| t.clone());
        let lines: Vec<MenuLine> = options.iter().map(|(_, t)| MenuLine { text: t.as_str().into(), enabled: true, rule: false, opens: false }).collect();
        let fractions: Vec<f32> = (0..5).map(|i| Self::slider_fraction(r, radius, i)).collect();
        let readouts: Vec<SharedString> = (0..5)
            .map(|i| match i {
                AMOUNT | TEMPERATURE | TINT => flat_field(i).spec().readout.format(Self::slider_value(r, radius, i)),
                PICKER => format!("{radius:.0} px"),
                _ => format!("{} / {}", r.at.min(last) + 1, m.len()),
            })
            .map(SharedString::from)
            .collect();
        let look = r.look();
        let base_note = r.base.as_ref().map_or("No base picked yet.".to_string(), |b| format!("On {}", oni_library::paths::name(&b.path)));
        let action = if step == Step::Window { measure_title(ticked - flagged) } else if filled(step, &r.flat, r.base.is_some()) { "NEXT".into() } else { "SKIP".into() };
        let heading = match measuring {
            Some((done, total)) => format!("Measuring {done} of {total}\u{2026}"),
            None => step.title().to_string(),
        };
        let passes_note = if r.capture == Capture::RgbPasses && frames.iter().all(|f| f.passes.is_some()) {
            format!("{} frames, grouped when the roll was last measured.", frames.len())
        } else {
            String::new()
        };
        let skipped: Vec<bool> = Step::ALL.iter().map(|s| s.skipped(r.capture)).collect();
        let (film, capture, flat, left_out, at, has_base) = (r.film, r.capture, r.flat.clone(), r.left_out, r.at, r.base.is_some());
        let shown_path = shown_frame.as_ref().map(|f| f.path.clone());
        let show_card = shown_path != r.shown_path;
        let base_path = (step == Step::Base).then(|| r.base_shown().map(|f| f.path.clone())).flatten();
        let ask_scan = base_path.is_some() && base_path != r.asked_scan;
        if let Some(previews) = &r.previews {
            previews.update(look);
            if show_card {
                previews.show(shown_path.as_deref());
            }
            if ask_scan {
                previews.scan(base_path.as_deref());
            }
        }
        if show_card {
            s.prepare.shown_path = shown_path;
        }
        if ask_scan {
            s.prepare.asked_scan = base_path;
        }
        drop(s);
        p.set_measuring(measuring.is_some());
        p.set_step(step.index() as i32);
        p.set_skipped(ModelRc::new(VecModel::from(skipped)));
        p.set_heading(heading.into());
        p.set_detail(if measuring.is_some() { SharedString::new() } else { step.detail().into() });
        p.set_status(status.into());
        p.set_differ(if measuring.is_some() { SharedString::new() } else { differ_text(&frames).into() });
        p.set_left_out(left_out_text(left_out).into());
        p.set_film(ConversionMode::ALL.iter().position(|f| *f == film).unwrap_or(0) as i32);
        p.set_capture(Capture::ALL.iter().position(|c| *c == capture).unwrap_or(1) as i32);
        p.set_passes_note(passes_note.into());
        p.set_flat_on(flat.mask != oni_settings::frame::FlatFieldMask::None);
        p.set_flat_masked(flat.mask != oni_settings::frame::FlatFieldMask::None && !(flat.mask == oni_settings::frame::FlatFieldMask::Custom && flat.custom.is_none()));
        p.set_monochrome(film.is_monochrome());
        p.set_masks_empty(masks.is_empty());
        p.set_mask_title(mask_title.into());
        p.set_mask_lines(ModelRc::new(VecModel::from(lines)));
        p.set_fractions(ModelRc::new(VecModel::from(fractions)));
        p.set_readouts(ModelRc::new(VecModel::from(readouts)));
        p.set_base_note(base_note.into());
        p.set_has_base(has_base);
        p.set_flags_note(format!("{} measured, {flagged} flagged", ticked - flagged).into());
        p.set_can_clear_flags(flagged > 0);
        p.set_frame_name(m.get(at).map(|f| oni_library::paths::name(&f.path)).unwrap_or_default().into());
        p.set_can_previous(at > 0);
        p.set_can_next(at < last);
        p.set_cells(ModelRc::new(VecModel::from(cells)));
        p.set_back_shown(step != Step::Film && measuring.is_none());
        p.set_action_title(action.into());
        p.set_action_enabled(step != Step::Window || ticked > flagged);
        p.set_card_picture(card);
        self.publish_card();
        self.publish_base();
    }

    /// The card's picture where its crop lies in the turned scan, the window on its own step.
    fn publish_card(&self) {
        let Some(w) = self.window() else { return };
        let Some(b) = self.card_box() else { return };
        let (crop, window, on_window) = {
            let r = &self.state.borrow().prepare;
            (r.shown_frame().map_or(Rect::UNIT, |f| f.crop), r.window, r.step == Step::Window)
        };
        let p = w.global::<Preparing>();
        let (x, y, pw, ph) = frames::shown(crop, b);
        p.set_picture_x(x as f32);
        p.set_picture_y(y as f32);
        p.set_picture_width(pw as f32);
        p.set_picture_height(ph as f32);
        p.set_window_shown(on_window);
        let (x, y, ww, wh) = frames::shown(window, b);
        p.set_window_x(x as f32);
        p.set_window_y(y as f32);
        p.set_window_width(ww as f32);
        p.set_window_height(wh as f32);
    }

    /// The base step's scan, its picked ring, and the loupe under the pointer.
    fn publish_base(&self) {
        let Some(w) = self.window() else { return };
        let p = w.global::<Preparing>();
        let (name, index, count) = {
            let r = &self.state.borrow().prepare;
            (r.base_shown().map(|f| oni_library::paths::name(&f.path)).unwrap_or_default(), r.base_frame.min(r.frames.len().saturating_sub(1)), r.frames.len())
        };
        p.set_base_name(name.into());
        p.set_base_can_previous(index > 0);
        p.set_base_can_next(index + 1 < count);
        let Some((rect, size)) = self.scan_rect() else {
            p.set_scan_ready(false);
            p.set_loupe_shown(false);
            return;
        };
        let s = self.state.borrow();
        let radius = base_radius(&s);
        let r = &s.prepare;
        let Some((path, scan, _)) = r.scan.clone() else { return };
        p.set_scan_ready(true);
        p.set_scan(scan.clone());
        p.set_scan_x(rect.0 as f32);
        p.set_scan_y(rect.1 as f32);
        p.set_scan_width(rect.2 as f32);
        p.set_scan_height(rect.3 as f32);
        let picked = r.base.as_ref().filter(|b| b.path == path);
        p.set_ring_shown(picked.is_some());
        if let Some(b) = picked {
            p.set_ring_x((rect.0 + b.unit.0 * rect.2) as f32);
            p.set_ring_y((rect.1 + b.unit.1 * rect.3) as f32);
            p.set_ring_diameter((2.0 * b.radius / size.0.max(1) as f64 * rect.2) as f32);
        }
        let Some(pointer) = r.pointer.filter(|(x, y)| *x >= rect.0 && *y >= rect.1 && *x <= rect.0 + rect.2 && *y <= rect.1 + rect.3) else {
            p.set_loupe_shown(false);
            return;
        };
        let whole = r.full.as_ref().filter(|(f, _, _)| *f == path);
        let (source, (width, height)) = whole.map_or((scan, size), |(_, i, s)| (i.clone(), *s));
        let l = loupe(pointer, rect, size.0, (width, height), whole.is_some(), radius, picked.map(|b| (b.unit, b.radius)));
        p.set_loupe_shown(true);
        p.set_loupe_x(pointer.0 as f32);
        p.set_loupe_y(pointer.1 as f32);
        p.set_loupe_source(source);
        p.set_loupe_cut(ModelRc::new(VecModel::from(l.cut.to_vec())));
        p.set_loupe_shown_at(ModelRc::new(VecModel::from(l.at.iter().map(|v| *v as f32).collect::<Vec<_>>())));
        p.set_loupe_rings(ModelRc::new(VecModel::from(l.rings.iter().map(|v| *v as f32).collect::<Vec<_>>())));
        p.set_loupe_picked(l.picked);
    }
}

/// The loupe's cut of the source in pixels, where it lands in points, and its rings: the pointer's
/// disc at x, y, diameter, then the picked one.
#[derive(Debug, PartialEq)]
pub struct LoupeLaid {
    pub cut: [i32; 4],
    pub at: [f64; 4],
    pub rings: [f64; 6],
    pub picked: bool,
}

/// `RollBaseSurface.loupe`: the pointer's pixel held at the centre past an edge too, nearest
/// neighbour over at least 64 source pixels, 32 on the display copy, or twice the disc.
pub fn loupe(pointer: (f64, f64), rect: (f64, f64, f64, f64), display_width: usize, source: (usize, usize), whole: bool, radius: f64, picked: Option<((f64, f64), f64)>) -> LoupeLaid {
    let side = 160.0;
    let (width, height) = (source.0 as f64, source.1 as f64);
    let display = display_width.max(1) as f64;
    let disc_pixels = 2.0 * radius * width / display;
    let least: f64 = if whole { 64.0 } else { 32.0 };
    let span = least.max((2.0 * disc_pixels).ceil());
    let unit = ((pointer.0 - rect.0) / rect.2.max(1.0), (pointer.1 - rect.1) / rect.3.max(1.0));
    let origin = ((unit.0 * width - span / 2.0).round(), (unit.1 * height - span / 2.0).round());
    let inside = oni_measure::window::intersection(&Rect::new(origin.0, origin.1, span, span), &Rect::new(0.0, 0.0, width, height));
    let scale = side / span;
    let place = |u: (f64, f64)| ((u.0 * width - origin.0) * scale, (u.1 * height - origin.1) * scale);
    let here = place(unit);
    let (picked_at, picked_disc) = picked.map_or(((0.0, 0.0), 0.0), |(u, r)| (place(u), 2.0 * r * width / display * scale));
    let empty = inside.width <= 0.0 || inside.height <= 0.0;
    LoupeLaid {
        cut: if empty { [0; 4] } else { [inside.x as i32, inside.y as i32, inside.width as i32, inside.height as i32] },
        at: [(inside.x - origin.0) * scale, (inside.y - origin.1) * scale, inside.width * scale, inside.height * scale],
        rings: [here.0, here.1, disc_pixels * scale, picked_at.0, picked_at.1, picked_disc],
        picked: picked.is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(path: &str) -> Frame {
        Frame::new(path, "")
    }

    /// `RollPicking.selfCheck`'s `ownRoll`: frames 0 to 4, references a, a, b, none, none.
    #[test]
    fn a_frame_brings_its_own_roll_and_every_unmeasured_frame_of_its_folder() {
        let (a, b) = (Id::new(), Id::new());
        let folder: Vec<String> = (0..5).map(|i| format!("/r/{i}")).collect();
        let reference = |p: &str| match p {
            "/r/0" | "/r/1" => Some(a),
            "/r/2" => Some(b),
            _ => None,
        };
        assert_eq!(own_roll("/r/0", &folder, reference), ["/r/0", "/r/1", "/r/3", "/r/4"]);
        assert_eq!(own_roll("/r/3", &folder, reference), ["/r/3", "/r/4"], "an unmeasured frame brings the unmeasured ones");
        // Twin: the frame of roll b brings b's and the unmeasured, never a's.
        assert_eq!(own_roll("/r/2", &folder, reference), ["/r/2", "/r/3", "/r/4"]);
        assert_ne!(own_roll("/r/2", &folder, reference), ["/r/0", "/r/3", "/r/4"]);
    }

    /// `RollPicking.selfCheck`'s `flagging`.
    #[test]
    fn a_click_flags_a_frame_a_second_unflags_it_and_flagging_ticks_it() {
        let mut frames = vec![frame("a"), frame("b")];
        flagging(&mut frames, 0);
        assert!(frames[0].flagged && !frames[1].flagged);
        flagging(&mut frames, 0);
        assert!(!frames[0].flagged, "a second click unflags");
        frames[1].ticked = false;
        flagging(&mut frames, 1);
        assert!(frames[1].flagged && frames[1].ticked, "the frame taken as flat-field comes back in");
        // Twin: unflagging leaves the frame ticked, never unticked.
        flagging(&mut frames, 1);
        assert!(!frames[1].flagged && frames[1].ticked);
    }

    #[test]
    fn the_steps_walk_past_the_flat_field_on_a_scanner_and_hold_at_either_end() {
        assert_eq!(neighbour(Step::Film, 1, Capture::Camera), Step::FlatField);
        assert_eq!(neighbour(Step::Film, 1, Capture::Scanner), Step::Base);
        assert_eq!(neighbour(Step::Base, -1, Capture::Scanner), Step::Film);
        assert_eq!(neighbour(Step::Window, 1, Capture::Camera), Step::Window);
        assert_eq!(neighbour(Step::Film, -1, Capture::Camera), Step::Film);
        // Twin: on a camera the flat-field is a step of its own.
        assert_ne!(neighbour(Step::Base, -1, Capture::Camera), Step::Film);
        assert_eq!(Step::ALL.map(Step::name), ["film", "flatField", "base", "flags", "window"]);
        assert_eq!(Step::named("flags"), Some(Step::Flags));
        assert_eq!(Capture::ALL.map(Capture::raw), ["scanner", "camera", "rgbPasses"]);
        let flat = FlatField { mask: oni_settings::frame::FlatFieldMask::ValoiEasy35V1, ..FlatField::default() };
        assert!(filled(Step::FlatField, &flat, false) && !filled(Step::FlatField, &FlatField::default(), false));
        assert!(!filled(Step::Base, &flat, false) && filled(Step::Base, &flat, true) && filled(Step::Flags, &FlatField::default(), false));
        assert_eq!((measure_title(1), measure_title(3)), ("MEASURE 1 FRAME".to_string(), "MEASURE 3 FRAMES".to_string()));
        assert_eq!(left_out_text(1), "1 frame of this folder, measured on another roll, is left out.");
        assert_eq!(left_out_text(2), "2 frames of this folder, measured on another roll, are left out.");
    }

    /// `RollFrameView.selfCheck`: a quarter-turned, cropped frame shows in its render's shape.
    #[test]
    fn a_quarter_turned_frame_shows_in_its_own_scans_shape() {
        let g = Geometry { quarter_turns: 1, crop: Rect::new(0.1, 0.2, 0.7, 0.5), ..Geometry::default() };
        let a = aspect(Some((300, 200)), &g).unwrap();
        let b = fitted(a, (1000.0, 1000.0));
        let shown = frames::shown(g.crop, b);
        let rendered = g.cropped_pixel_size((300.0, 200.0));
        let ratio = (shown.2 / shown.3) / (rendered.0 / rendered.1);
        assert!((ratio - 1.0).abs() < 0.01, "shown {shown:?}, rendered {rendered:?}");
        // Twin: in the unturned shape the same crop is off by more than half.
        let wrong = frames::shown(g.crop, fitted(1.5, (1000.0, 1000.0)));
        assert!(((wrong.2 / wrong.3) / (rendered.0 / rendered.1) - 1.0).abs() > 0.5);
    }

    #[test]
    fn a_draft_resumes_its_step_and_leaves_out_the_frames_of_none() {
        let mut r = Roll { frames: vec![frame("a"), frame("b"), frame("c")], ..Roll::default() };
        let id = Id::new();
        let draft = RollDraft {
            id,
            step: "flags".into(),
            film: ConversionMode::Positive,
            capture: "scanner".into(),
            pass_order: None,
            flat_field: None,
            base: Some(DraftBase { path: "b".into(), x: 0.1, y: 0.2, radius: 3.0 }),
            window: None,
            ticked: true,
            flagged: true,
        };
        r.resume(None, 0);
        assert!(r.frames[0].ticked, "with no draft yet a frame keeps its tick");
        r.resume(Some(&draft), 1);
        assert_eq!((r.step, r.film, r.capture), (Step::Flags, ConversionMode::Positive, Capture::Scanner));
        assert_eq!(r.base, Some(BasePick { path: "b".into(), unit: (0.1, 0.2), radius: 3.0 }));
        assert!(!r.frames[0].ticked, "a frame landed before the draft, of none, is left out");
        assert!(r.frames[1].ticked && r.frames[1].flagged);
        let other = RollDraft { id: Id::new(), ..draft.clone() };
        r.resume(Some(&other), 2);
        assert!(!r.frames[2].ticked, "twin: another draft's frame is left out");
        // The draft written back carries each frame's own place; none before every frame is read.
        r.shown = true;
        assert!(r.drafts().is_none());
        for f in ["a", "b", "c"] {
            r.read.insert(f.into(), PipelineSettings::default());
        }
        let drafts = r.drafts().unwrap();
        assert_eq!(drafts.iter().map(|(_, d)| (d.ticked, d.flagged, d.id)).collect::<Vec<_>>(), [(false, false, id), (true, true, id), (false, false, id)]);
        assert_eq!(drafts[0].1.step, "flags");
    }

    #[test]
    fn the_loupe_holds_the_pointers_pixel_at_its_centre_past_an_edge() {
        // A 1400 px display copy drawn 700 points wide; the pointer at its very left edge.
        let laid = loupe((10.0, 60.0), (10.0, 10.0, 700.0, 466.0), 1400, (1400, 932), false, 4.0, None);
        assert_eq!(laid.rings[2], 2.0 * 4.0 * 160.0 / 32.0, "an 8 px disc over a 32 px span");
        assert!((laid.rings[0] - 80.0).abs() < 1e-9, "the pointer's pixel stays at the centre: {laid:?}");
        assert!(laid.at[0] > 0.0, "the canvas shows left of the scan's edge: {laid:?}");
        // Twin: on the full-size source the span is 64 pixels, not 32, until twice the disc is wider.
        let full = loupe((360.0, 243.0), (10.0, 10.0, 700.0, 466.0), 1400, (6000, 4000), true, 2.0, None);
        assert_eq!(full.cut[2], 64);
        assert!(full.cut[2] != laid.cut[2]);
        let wide = loupe((360.0, 243.0), (10.0, 10.0, 700.0, 466.0), 1400, (6000, 4000), true, 4.0, None);
        assert_eq!(wide.cut[2], 69, "an 8 px disc is 34.3 source pixels, twice that 69");
    }

    #[test]
    fn the_rolls_settings_differ_note_counts_the_frames_shot_apart() {
        let c = CaptureExposure { shutter: 1.0 / 50.0, aperture: 5.6, iso: 100.0 };
        let mut frames = vec![frame("a"), frame("b"), frame("c")];
        frames[0].capture = Some(c);
        frames[1].capture = Some(c);
        frames[2].capture = Some(CaptureExposure { aperture: 8.0, ..c });
        assert_eq!(
            differ_text(&frames),
            "1 of 3 frames, marked, were digitised with other settings than the roll's 1/50 s, f/5.6, ISO 100. Their exposure is evened out to it; a different aperture also changes the vignetting the flat-field measured, so re-shoot them in manual for the cleanest roll."
        );
        // Twin: a frame whose EXIF reads nothing is never apart.
        frames[2].capture = None;
        assert_eq!(differ_text(&frames), "");
    }
}
