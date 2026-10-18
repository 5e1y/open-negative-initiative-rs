//! The window and what drives it: the sliders' timing, the store's publishing, the canvas's render
//! loop, the gallery's visible tiles, the tree, the bar, the menus and the keys.

use crate::display::{self, Params, Screen};
use crate::fields::{Block, Field};
use crate::gallery::{self, Layout, Metrics};
use crate::library::{self, Archive};
use crate::preferences::{self, Preferences};
use crate::thumbnails::{self, Memory};
use crate::render::{self, Done, Histogram, Job, Negative, Picture, Placed, Previews, Shared, View, Worker};
use crate::viewport::{self, Viewport};
use crate::framing::{self as frames, Handle};
use oni_settings::cg::Rect as Fraction;
use oni_settings::frame::Geometry;
use oni_settings::geometry::{self as geom, Framing as Remembered};
use crate::phone::{self, Phone};
use crate::store::{self, Store as Settings};
use crate::timing::{self, Kind, Notches};
use crate::{App, Bar, Canvas, Confirm, Heading, Mark, Library, MainWindow, MenuLive, MenuTitles, Menus, Pairing, PanelState, Plots, Store as StoreGlobal, Tile, TreeRow};
use oni_core::{Command, PasteCheck, Surface};
use oni_settings::ConversionMode;
use oni_core::link::Outward;
use oni_link::HostEvent;
use crate::motion::Spring;
use crate::system;
use crate::track::{self, Hand, Offer, TRACKPAD_TRAVEL};
use slint::{ComponentHandle, Image, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, Timer, TimerMode, VecModel};
use std::cell::RefCell;
use oni_library::gallery_history::Move;
use oni_library::index::LibraryIndex;
use oni_library::roots::Survey;
use oni_settings::Medal;
use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};

#[path = "export_card.rs"]
pub mod export_card;

/// Long side of a gallery thumbnail, in pixels: `Thumbnails.side`.
pub const THUMB_SIDE: usize = thumbnails::SIDE;
const FRAME_TICK: Duration = Duration::from_millis(16);
/// What changes on its own rhythm, histogram and load counter, is published at this pace.
const PULSE: Duration = Duration::from_millis(100);
/// `DS.Control.cropHint` and `DS.Space.md`: the crop badge's margin inside the view and its rise above the frame.
const BADGE_MARGIN: f64 = 32.0;
const BADGE_RISE: f64 = 16.0;
/// `DSScroll.idleWindow`: a pause this long ends a straightening gesture.
const STRAIGHTEN_IDLE: Duration = Duration::from_millis(160);
/// `Sidecar.writeDelay`: the hand rests this long before the sidecar is written.
pub const WRITE_DELAY: Duration = oni_settings::sidecar::WRITE_DELAY;
/// `DSHistogram`'s raster: the panel's width at 2x by the plot's height at 2x.
const HISTOGRAM: (usize, usize) = (680, 300);

mod lab;
mod organising;
mod presets;
mod roll_tab;
mod rolls;
/// The folder picker `Open Folder…` shows; a check hands its own.
pub type Picker = Box<dyn Fn() -> Option<PathBuf>>;
/// Where a deleted preset file goes: the system's Trash, or a check's own folder.
pub type Discard = Box<dyn Fn(&Path) -> std::io::Result<()>>;

pub struct Options {
    pub screen: Screen,
    /// The device Slint draws the window with, on which the display kernel writes the canvas;
    /// none on the software renderer.
    pub canvas: Option<(wgpu::Device, wgpu::Queue)>,
    /// `AppFolders.support`: the phone link's keys, the preferences, the thumbnails.
    pub support: PathBuf,
    /// `AppFolders.caches`: the index, which the system may purge.
    pub caches: PathBuf,
    pub picker: Picker,
    pub discard: Discard,
}

/// `EditorScene.open`'s panel message.
pub const PICKER_MESSAGE: &str = "Choose a folder: every photo under it shows in the archive. Nothing is copied or moved.";

impl Default for Options {
    fn default() -> Self {
        Options {
            screen: Screen::Srgb,
            canvas: None,
            support: timing::support(),
            caches: system::caches(),
            picker: Box::new(|| system::pick_folder(PICKER_MESSAGE)),
            discard: Box::new(system::trash),
        }
    }
}

/// The thumbnails pass and the edits' redraws, one render at a time for the app, a redraw first.
#[derive(Default)]
struct Paint {
    /// An edit's redraws by path, with the fingerprint the index knows.
    redraws: VecDeque<(String, Option<String>)>,
    /// The pass a folder asked last, by generation; it replaces the one running.
    pass: Option<(u64, Vec<(String, String)>)>,
}

type Painter = Arc<(Mutex<Paint>, Condvar)>;

/// The Crop tab, which ⌘K opens.
const CROP_TAB: i32 = 1;
/// `pasteSettings`'s dialog when a look crosses rolls.
const ANOTHER_ROLL: (&str, &str) = ("Another Roll", "You are trying to apply settings from one roll to another: results may vary.");

/// A gesture held until the confirm sheet's key answers.
enum Pending {
    Paste(Vec<String>),
    PasteToRoll,
    DeleteProject(oni_settings::Id),
    RestoreAll,
    DeletePreset(oni_settings::Id),
    UpdatePreset(oni_settings::Id),
    ResetToDefault(oni_library::roll_membership::ResetReach),
    QuitAnyway,
}

/// A render on its way to the screen, timed from its ask.
struct Shown {
    kind: Kind,
    asked: Instant,
    pipeline: Duration,
    frame: String,
}

/// One slider's hand: the dragged value drawn at once, handed to the store thirty times a second.
#[derive(Default)]
struct Slide {
    live: Option<f32>,
    start: Option<f32>,
    resetting: bool,
    last_press: Option<Instant>,
    hand: Hand,
    /// Where the knob is drawn, as a fraction of the track.
    shown: f32,
    arrival: Option<(f32, f32, Instant)>,
    /// The stored value last published, so a field holding several sliders publishes only the moved one.
    published: Option<f32>,
    /// When the hand last moved the value, which the notch showing it is timed from.
    moved_at: Option<Instant>,
}

/// The display kernel and the texture it writes, once the renderer hands over its wgpu device.
struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    kernel: display::Gpu,
    target: Option<wgpu::Texture>,
    /// Compare's before, encoded beside the after, and the render it was encoded from.
    before: Option<(wgpu::Texture, oni_graph::Source)>,
    /// The picture the canvas was last encoded from, alive until the next one is encoded.
    current: Option<Picture>,
    /// A frame was encoded since the histogram last counted one.
    uncounted: bool,
    seed: u32,
}

impl Gpu {
    /// The histogram counted on a frame already shown, once its readback has landed; then the
    /// newest frame is counted, so the GPU counts at the pulse's pace and never on a notch.
    fn landed(&mut self) -> Option<Histogram> {
        let _ = self.device.poll(wgpu::PollType::Poll);
        let landed = self.kernel.histogram();
        if self.uncounted
            && let Some(target) = &self.target
            && self.kernel.count(&self.device, &self.queue, target)
        {
            self.uncounted = false;
        }
        landed
    }
}

pub struct State {
    pub(crate) store: Box<dyn Settings>,
    screen: Screen,
    archive: Option<Archive>,
    /// Every photo's path in gallery order.
    order: Vec<String>,
    /// Each photo's fingerprint, which names its thumbnail's file.
    fingerprints: HashMap<String, String>,
    /// `opened`: the folder the archive shows, remembered under `openedFolder`.
    opened: Option<String>,
    preferences: Preferences,
    /// The interface thread's own connection to the index; a read opens its own.
    index: Option<LibraryIndex>,
    index_path: PathBuf,
    survey: Survey,
    /// Bumped by every folder opened, which stops the previous folder's read and pass.
    generation: Arc<AtomicU64>,
    read_done: bool,
    /// A file to open in the editor once its folder is read, as a dropped file is.
    opening: Option<String>,
    filter: Option<Medal>,
    painter: Painter,
    disk: mpsc::Sender<String>,
    /// Thumbnails asked of the folder and not landed yet.
    asked: HashSet<String>,
    /// The pass's progress: photos settled, of those the folder holds.
    painted: (usize, usize),
    /// The pass's photos not in the folder, drawn of those it found absent: the status line.
    creating: (usize, usize),
    /// Fingerprints a render could not draw, until an edit or the next pass.
    failed: HashSet<String>,
    /// Redraws asked and not landed.
    drawing: usize,
    /// Every row the index shows under the opened folder, before the filter.
    rows: Vec<oni_library::Entry>,
    /// The photo the editor holds, which need not be under the opened folder.
    open: Option<String>,
    /// What a confirm sheet goes ahead with once its key is pressed.
    pending: Option<Pending>,
    muted: bool,
    /// The gallery's picks, kept by path so a relayout keeps them, and the ONI Trash's own.
    picks: crate::organise::Picks,
    trash_picks: crate::organise::Picks,
    /// The archive tree's rows as published, each photo row's path, so a pick relights them in place.
    tree_paths: Vec<Option<String>>,
    shelf: presets::Shelf,
    /// The balance key's run under way, which the key shows instead of itself.
    batch: Option<rolls::Batch>,
    org: organising::Organising,
    columns: usize,
    scroll: f32,
    grid: (f32, f32),
    layout: Layout,
    visible: std::ops::Range<usize>,
    thumbs: Memory<Image>,
    /// The open frame's preview, which the canvas renders; the previews kept besides it.
    negative: Option<(String, Arc<Negative>)>,
    previews: Arc<Mutex<Previews>>,
    /// `DSGesture`: hands on a control, which hold the thumbnails' decodes back.
    pub(crate) hands: Arc<AtomicUsize>,
    slides: Vec<Slide>,
    rendered: u64,
    requested: u64,
    histogram: Option<Histogram>,
    histogram_dirty: bool,
    /// The levels' and manual curves' input distributions last read, and the focus each was drawn with.
    inputs: Option<render::InputHistograms>,
    inputs_drawn: Option<(Option<usize>, Option<usize>)>,
    last_change: Option<Instant>,
    /// The store revision `publish` last saw, so any edit of the open frame arms its write.
    published_revision: u64,
    /// How the open frame's preview was decoded: a sidecar naming another reading decodes it again.
    reading: Option<render::Reading>,
    /// The window may close: a failed write was answered with Quit Anyway.
    quitting: bool,
    /// The flat-field masks loaded by hand, read once and again after each change to them.
    masks: Vec<crate::flat_fields::Entry>,
    status: String,
    brush: f32,
    gpu: Option<Gpu>,
    renders: usize,
    opened_at: Instant,
    notches: Notches,
    /// When the hand moved the value the next render shows; `None` times a render from its ask.
    moved_at: Option<Instant>,
    viewport: Viewport,
    views: Notches,
    /// The open frame's full-resolution pixels once built, and the frame one is being built for.
    full: Option<(String, oni_graph::Source)>,
    building_full: Option<String>,
    /// The frame whose dust patches are being generated.
    healing: Option<String>,
    /// The last navigation, which keeps the preview until it settles.
    navigated_at: Option<Instant>,
    /// The displayed image's size in full-resolution pixels, from the last render placed.
    full_size: Option<(f64, f64)>,
    /// What the last render request showed, so a navigation alone asks again.
    requested_view: Option<(Viewport, (f64, f64), bool, bool, bool, bool)>,
    /// The settings the last render was asked with, so a crop moved while framing renders nothing.
    requested_live: Option<oni_settings::PipelineSettings>,
    placed: Option<Placed>,
    grab: Option<(f32, f32)>,
    pinch: f32,
    /// The ratio and orientation last chosen, and the frame chosen on, handed to the next frame framed.
    framing_memory: Option<(Remembered, String)>,
    /// A frame held on the canvas: the window's or the crop's, the handle, the frame at the press
    /// and the shown image frame's size in points.
    frame_gesture: Option<(bool, Handle, Fraction, (f64, f64))>,
    /// The frame a straightening gesture set out from, the one it last wrote, and when.
    straightening: Option<(Geometry, Geometry, Instant)>,
    window_armed: bool,
    /// The panel the hand was in before De-dust, where leaving it lands unless it was the crop.
    last_panel: i32,
    /// The stroke being painted, in fractions of the rendered frame, y down.
    painting_stroke: Vec<(f64, f64)>,
    /// The number each correction's marker shows, never its row, which a removal shifts.
    numbers: HashMap<oni_settings::id::Id, usize>,
    next_number: usize,
    /// Where each correction's patch was copied from, in fractions of the rendered frame, y down.
    sources: HashMap<oni_settings::id::Id, (f64, f64)>,
    /// The corrections whose search found no match and laid the tile beside the stroke.
    fallback: std::collections::HashSet<oni_settings::id::Id>,
    brush_start: Option<f32>,
    brush_pressed_at: Option<Instant>,
    /// `Propagation`'s copied measuring window.
    window_board: Option<Fraction>,
    /// Compare Before/After's split, a fraction of the canvas's width; none while off.
    split: Option<f32>,
    /// `screenGamutWarning`: the pixels this screen cannot show, striped on the canvas.
    screen_gamut: bool,
    before_placed: Option<Placed>,
    /// How many distinct befores the canvas encoded, which a kept one does not add to.
    befores: usize,
    support: PathBuf,
    phone: Option<Phone>,
    picker: Picker,
    discard: Discard,
}

impl State {
    /// The gallery's picks as places in its order, which a batch walks.
    pub(crate) fn selection(&self) -> std::collections::BTreeSet<usize> {
        self.order.iter().enumerate().filter(|(_, p)| self.picks.selection.contains(*p)).map(|(i, _)| i).collect()
    }

    /// `TreeEngine.marked`: the selection the gallery shares, and the ring (`TreeEngine.swift:247`).
    fn tree_marks(&self, path: &str) -> bool {
        self.picks.selection.contains(path) || self.picks.anchor.as_deref() == Some(path)
    }

    /// `designate(_:)`, which every load runs: the frame opened is the selection, cursor and ring.
    fn designate(&mut self, path: &str) {
        self.picks.selection = HashSet::from([path.to_string()]);
        self.picks.anchor = Some(path.to_string());
        self.picks.cursor = Some(path.to_string());
    }

    /// The settings the canvas was last asked to show, a hovered preset's preview included.
    pub fn shown_settings(&self) -> Option<&oni_settings::PipelineSettings> {
        self.requested_live.as_ref()
    }

    pub fn renders(&self) -> usize {
        self.renders
    }

    /// The store revision the canvas last showed.
    pub fn rendered(&self) -> u64 {
        self.rendered
    }

    pub fn store(&self) -> &dyn Settings {
        self.store.as_ref()
    }

    pub fn open_path(&self) -> Option<&str> {
        self.open.as_deref()
    }

    /// The thumbnails of the opened folder settled so far: held in the folder, drawn, or refused.
    pub fn loaded(&self) -> usize {
        self.painted.0
    }

    /// Whether the opened folder's read has landed whole.
    pub fn read_done(&self) -> bool {
        self.read_done
    }

    pub fn opened(&self) -> Option<&str> {
        self.opened.as_deref()
    }

    pub fn filter(&self) -> Option<Medal> {
        self.filter
    }

    /// The thumbnails in memory dropped, as a long scroll drops them, so a check sees a tile read
    /// back from the folder.
    pub fn forget_tiles(&mut self) {
        self.thumbs.clear();
        self.asked.clear();
    }

    /// The photo's fingerprint as the index holds it, which names its thumbnail's file.
    pub fn fingerprint(&self, path: &str) -> Option<&str> {
        self.fingerprints.get(path).map(String::as_str)
    }

    /// Whether a thumbnail redraw is queued or drawing: an edit's, never the pass's.
    pub fn drawing(&self) -> bool {
        self.drawing > 0
    }

    /// The photos the open folder holds, which the thumbnails count up to.
    pub fn photos(&self) -> usize {
        self.order.len()
    }

    pub fn visible(&self) -> std::ops::Range<usize> {
        self.visible.clone()
    }

    pub fn brush(&self) -> f32 {
        self.brush
    }

    /// The notch renders shown so far, the index `Notches::summary` starts from.
    pub fn notches(&self) -> &Notches {
        &self.notches
    }

    /// The zooms and pans shown, timed as the notches are.
    pub fn views(&self) -> &Notches {
        &self.views
    }

    pub fn placed(&self) -> Option<Placed> {
        self.placed
    }

    pub fn viewport(&self) -> Viewport {
        self.viewport
    }

    pub fn split(&self) -> Option<f32> {
        self.split
    }

    pub fn befores(&self) -> usize {
        self.befores
    }

    pub fn phone(&self) -> Option<&Phone> {
        self.phone.as_ref()
    }

    /// `loadBar`: the thumbnails being created, else the library being read, with their counts.
    pub fn load_bar(&self) -> Option<String> {
        let (done, total) = self.creating;
        if done < total {
            return Some(format!("Creating thumbnails · {done} of {total}"));
        }
        if !self.read_done && self.opened.is_some() {
            let waiting = self.rows.iter().filter(|e| e.fingerprint.starts_with(thumbnails::PENDING)).count();
            let total = self.rows.len();
            return Some(format!("Reading the library · {} of {total}", total - waiting));
        }
        None
    }

    /// The open frame's place in its group, one-based, and the group's count.
    fn place(&self) -> Option<(u16, u16)> {
        let open = self.open_path()?;
        let group = self.archive.as_ref()?.groups.iter().find(|g| g.entries.iter().any(|e| e.path == open))?;
        let at = group.entries.iter().position(|e| e.path == open)?;
        Some((at as u16 + 1, group.entries.len() as u16))
    }
}

pub struct Ctl {
    window: slint::Weak<MainWindow>,
    pub state: RefCell<State>,
    engine: Shared,
    worker: Worker<Negative>,
    trailing: Vec<Timer>,
    arrivals: Vec<Timer>,
    pulse: Timer,
    settle: Timer,
    stripes: Timer,
    pub(crate) editors: crate::editors::Shell,
    /// `Thumbnails.refreshDelay` after the sidecar's write, the open photo's thumbnail is redrawn.
    refresher: Timer,
    /// Restarted by every change, so the sidecar is written once the hand has rested `WRITE_DELAY`.
    writer: Timer,
    export: RefCell<export_card::Card>,
}

thread_local! {
    /// The interface thread's shell, which background threads reach through `invoke_from_event_loop`.
    static SHELL: RefCell<Weak<Ctl>> = const { RefCell::new(Weak::new()) };
}

fn with_shell(f: impl FnOnce(&Rc<Ctl>) + Send + 'static) {
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(c) = SHELL.with(|s| s.borrow().upgrade()) {
            f(&c);
        }
    });
}

macro_rules! fields {
    ($($variant:ident => $fraction:ident, $get:ident, $readout:ident, $value:ident;)*) => {
        fn set_fraction(g: &StoreGlobal, f: Field, v: f32) {
            match f { $(Field::$variant => g.$fraction(v),)* }
        }
        /// The fraction the panel draws, read back by the checks.
        pub fn fraction(g: &StoreGlobal, f: Field) -> f32 {
            match f { $(Field::$variant => g.$get(),)* }
        }
        fn set_readout(g: &StoreGlobal, f: Field, v: SharedString) {
            match f { $(Field::$variant => g.$readout(v),)* }
        }
        fn set_value(g: &StoreGlobal, f: Field, v: SharedString) {
            match f { $(Field::$variant => g.$value(v),)* }
        }
    };
}

fields! {
    Temperature => set_temperature_fraction, get_temperature_fraction, set_temperature_readout, set_temperature_value;
    Tint => set_tint_fraction, get_tint_fraction, set_tint_readout, set_tint_value;
    ColorDensity => set_color_density_fraction, get_color_density_fraction, set_color_density_readout, set_color_density_value;
    Exposure => set_exposure_fraction, get_exposure_fraction, set_exposure_readout, set_exposure_value;
    Highlights => set_highlights_fraction, get_highlights_fraction, set_highlights_readout, set_highlights_value;
    Shadows => set_shadows_fraction, get_shadows_fraction, set_shadows_readout, set_shadows_value;
    WhitePoint => set_white_point_fraction, get_white_point_fraction, set_white_point_readout, set_white_point_value;
    BlackPoint => set_black_point_fraction, get_black_point_fraction, set_black_point_readout, set_black_point_value;
    Contrast => set_contrast_fraction, get_contrast_fraction, set_contrast_readout, set_contrast_value;
    Vignette => set_vignette_fraction, get_vignette_fraction, set_vignette_readout, set_vignette_value;
    Chrome => set_chrome_fraction, get_chrome_fraction, set_chrome_readout, set_chrome_value;
    BalanceStrength => set_balance_strength_fraction, get_balance_strength_fraction, set_balance_strength_readout, set_balance_strength_value;
    DensityCeiling => set_density_ceiling_fraction, get_density_ceiling_fraction, set_density_ceiling_readout, set_density_ceiling_value;
    DyeSeparation => set_dye_separation_fraction, get_dye_separation_fraction, set_dye_separation_readout, set_dye_separation_value;
    Angle => set_angle_fraction, get_angle_fraction, set_angle_readout, set_angle_value;
    SharpenAmount => set_sharpen_amount_fraction, get_sharpen_amount_fraction, set_sharpen_amount_readout, set_sharpen_amount_value;
    SharpenRadius => set_sharpen_radius_fraction, get_sharpen_radius_fraction, set_sharpen_radius_readout, set_sharpen_radius_value;
    SharpenHalo => set_sharpen_halo_fraction, get_sharpen_halo_fraction, set_sharpen_halo_readout, set_sharpen_halo_value;
    ChromaAmount => set_chroma_amount_fraction, get_chroma_amount_fraction, set_chroma_amount_readout, set_chroma_amount_value;
    ChromaRadius => set_chroma_radius_fraction, get_chroma_radius_fraction, set_chroma_radius_readout, set_chroma_radius_value;
    Texture => set_texture_fraction, get_texture_fraction, set_texture_readout, set_texture_value;
    PaperRed => set_paper_red_fraction, get_paper_red_fraction, set_paper_red_readout, set_paper_red_value;
    PaperGreen => set_paper_green_fraction, get_paper_green_fraction, set_paper_green_readout, set_paper_green_value;
    PaperBlue => set_paper_blue_fraction, get_paper_blue_fraction, set_paper_blue_readout, set_paper_blue_value;
    GainRed => set_gain_red_fraction, get_gain_red_fraction, set_gain_red_readout, set_gain_red_value;
    GainGreen => set_gain_green_fraction, get_gain_green_fraction, set_gain_green_readout, set_gain_green_value;
    GainBlue => set_gain_blue_fraction, get_gain_blue_fraction, set_gain_blue_readout, set_gain_blue_value;
    FlatAmount => set_flat_amount_fraction, get_flat_amount_fraction, set_flat_amount_readout, set_flat_amount_value;
    FlatTemperature => set_flat_temperature_fraction, get_flat_temperature_fraction, set_flat_temperature_readout, set_flat_temperature_value;
    FlatTint => set_flat_tint_fraction, get_flat_tint_fraction, set_flat_tint_readout, set_flat_tint_value;
}

fn set_modified(g: &StoreGlobal, b: Block, on: bool) {
    match b {
        Block::Color => g.set_color_modified(on),
        Block::Lighting => g.set_lighting_modified(on),
        Block::Effects => g.set_effects_modified(on),
        Block::BalanceStrength => g.set_balance_strength_modified(on),
        Block::DensityCeiling => g.set_density_ceiling_modified(on),
        Block::DyeSeparation => g.set_dye_separation_modified(on),
        Block::Sharpening => g.set_sharpening_modified(on),
        Block::ChromaDenoise => g.set_chroma_denoise_modified(on),
        Block::Texture => g.set_texture_modified(on),
        Block::PaperCorrection => g.set_paper_correction_modified(on),
        Block::SourceBalance => g.set_source_balance_modified(on),
        Block::FlatField => g.set_flat_field_modified(on),
    }
}

fn track_range(f: Field) -> RangeInclusive<f32> {
    let (low, high) = f.track();
    low..=high
}

/// Where a stored value sits on its track.
fn position(f: Field, stored: f32) -> f32 {
    track::position(f.shown(stored), &track_range(f), f.spec().centre)
}

/// The stored value at a track position: the exact inverse of `position`.
fn stored_at(f: Field, at: f32) -> f32 {
    f.shown(track::value(at, &track_range(f), f.spec().centre))
}

fn image(bytes: &[u8], w: usize, h: usize) -> Image {
    Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(bytes, w as u32, h as u32))
}

fn premultiplied(bytes: &[u8], w: usize, h: usize) -> Image {
    Image::from_rgba8_premultiplied(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(bytes, w as u32, h as u32))
}

fn stem(path: &str) -> String {
    Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

pub struct Shell {
    pub window: MainWindow,
    pub ctl: Rc<Ctl>,
}

impl Shell {
    /// The window over `engine`, whose device the canvas's texture must share when Slint draws on wgpu.
    pub fn new(store: Box<dyn Settings>, engine: Shared, options: Options) -> Result<Shell, slint::PlatformError> {
        let window = MainWindow::new()?;
        let worker = Worker::new(Box::new(engine.clone()), options.screen, |done| with_shell(move |c| c.deliver(done)));
        worker.on_gpu.store(options.canvas.is_some(), Ordering::Relaxed);
        let index_path = LibraryIndex::default_path(&options.caches);
        let painter: Painter = Arc::default();
        let previews: Arc<Mutex<Previews>> = Arc::default();
        let hands: Arc<AtomicUsize> = Arc::default();
        let generation: Arc<AtomicU64> = Arc::default();
        paint(painter.clone(), engine.clone(), previews.clone(), hands.clone(), generation.clone(), options.support.clone(), options.screen);
        let disk = read_disk(options.support.clone(), options.screen);
        let state = State {
            store,
            screen: options.screen,
            archive: None,
            order: Vec::new(),
            fingerprints: HashMap::new(),
            opened: None,
            preferences: Preferences::load(&options.support),
            index: LibraryIndex::open(&index_path),
            index_path,
            survey: Survey::default(),
            generation,
            read_done: false,
            opening: None,
            filter: None,
            painter,
            disk,
            asked: HashSet::new(),
            painted: (0, 0),
            creating: (0, 0),
            failed: HashSet::new(),
            drawing: 0,
            rows: Vec::new(),
            pending: None,
            muted: false,
            open: None,
            picks: Default::default(),
            trash_picks: Default::default(),
            tree_paths: Vec::new(),
            shelf: Default::default(),
            batch: None,
            org: Default::default(),
            columns: gallery::COLUMN_STEPS[1],
            scroll: 0.0,
            grid: (0.0, 0.0),
            layout: Layout::new(&[], 6, 0.0, Metrics::GALLERY),
            visible: 0..0,
            thumbs: Memory::new(thumbnails::MEMORY_LIMIT),
            negative: None,
            previews,
            hands,
            slides: Field::ALL.iter().map(|_| Slide::default()).collect(),
            rendered: 0,
            requested: 0,
            histogram: None,
            histogram_dirty: false,
            inputs: None,
            inputs_drawn: None,
            last_change: None,
            published_revision: 0,
            reading: None,
            quitting: false,
            masks: crate::flat_fields::entries(&options.support),
            status: String::new(),
            brush: oni_settings::frame::Correction::DEFAULT_RADIUS as f32,
            // No rendering notifier: Slint flushes femtovg before calling it, which empties femtovg's
            // pipeline cache, so every frame recompiled its pipelines (8 ms of the interface thread on Metal).
            gpu: options.canvas.map(|(device, queue)| {
                let kernel = display::Gpu::new(&device);
                Gpu { device, queue, kernel, target: None, before: None, current: None, uncounted: false, seed: 0 }
            }),
            renders: 0,
            opened_at: Instant::now(),
            notches: Notches::default(),
            moved_at: None,
            viewport: Viewport::default(),
            views: Notches::default(),
            full: None,
            building_full: None,
            healing: None,
            navigated_at: None,
            full_size: None,
            requested_view: None,
            requested_live: None,
            placed: None,
            grab: None,
            pinch: 1.0,
            split: None,
            framing_memory: None,
            frame_gesture: None,
            straightening: None,
            window_armed: false,
            last_panel: 2,
            painting_stroke: Vec::new(),
            numbers: HashMap::new(),
            next_number: 1,
            fallback: std::collections::HashSet::new(),
            sources: HashMap::new(),
            brush_start: None,
            brush_pressed_at: None,
            window_board: None,
            befores: 0,
            screen_gamut: false,
            before_placed: None,
            support: options.support.clone(),
            phone: None,
            picker: options.picker,
            discard: options.discard,
        };
        let ctl = Rc::new(Ctl {
            window: window.as_weak(),
            state: RefCell::new(state),
            engine,
            worker,
            trailing: Field::ALL.iter().map(|_| Timer::default()).collect(),
            arrivals: Field::ALL.iter().map(|_| Timer::default()).collect(),
            pulse: Timer::default(),
            settle: Timer::default(),
            stripes: Timer::default(),
            editors: crate::editors::Shell::default(),
            refresher: Timer::default(),
            writer: Timer::default(),
            export: RefCell::new(export_card::Card::new(&options.support)),
        });
        SHELL.with(|s| *s.borrow_mut() = Rc::downgrade(&ctl));
        Ctl::wire(&ctl, &window);
        Ctl::wire_export(&ctl, &window);
        ctl.attach_export();
        ctl.restore_gamut();
        // The app opens on the gallery; nothing opens in the editor until a hand asks.
        window.global::<App>().set_gallery(true);
        ctl.publish(true);
        ctl.publish_bar();
        // A phone paired before finds the desktop listening again; nothing pairs until asked.
        if phone::has_trusted(&options.support) {
            ctl.start_phone();
        }
        Ok(Shell { window, ctl })
    }

    /// `openFolder`: the gallery on the folder, read through the index behind, its thumbnails
    /// drawn by the pass; nothing opens in the editor until a hand asks.
    pub fn open_folder(&self, folder: &Path) {
        self.ctl.open_folder(folder);
    }

    /// `add(_:)`: a folder opens; a file opens its folder, then itself in the editor once found.
    pub fn add(&self, path: &Path) {
        self.ctl.add(path);
    }

    /// The launch's folder: the one last opened, else the first root of `roots.json`, else none.
    pub fn open_remembered(&self) -> bool {
        self.ctl.open_remembered()
    }

    /// The opened folder's first photo in the editor once read, for the bench.
    pub fn open_first_when_read(&self) {
        self.ctl.state.borrow_mut().opening = Some(String::new());
    }

    /// The open frame's sidecar written as the app ends, whatever ended it; the failure, if any.
    pub fn flush(&self) -> Option<String> {
        self.ctl.state.borrow_mut().store.flush()
    }

    /// The session's notch renders, for the line printed on exit.
    pub fn summary(&self) -> String {
        self.ctl.state.borrow().notches.summary(0, "session")
    }
}

impl Ctl {
    pub(crate) fn window(&self) -> Option<MainWindow> {
        self.window.upgrade()
    }

    // MARK: - Publishing

    /// Every moved slider's value to the panel; a knob under the hand keeps its place, one moved by
    /// a command springs there, and `jump` places them all at once, as opening a photo does.
    pub(crate) fn publish(self: &Rc<Self>, jump: bool) {
        let Some(w) = self.window() else { return };
        let g = w.global::<StoreGlobal>();
        let (moved, edited) = {
            let mut s = self.state.borrow_mut();
            // Any new state of the open frame is written, a slider's or not: a roll joined or reset moves none.
            let revision = s.store.revision();
            let edited = revision != s.published_revision;
            s.published_revision = revision;
            let mut moved: Vec<Field> = s.store.changed();
            if jump {
                moved = Field::ALL.to_vec();
            }
            let mut out = Vec::new();
            for f in moved {
                let v = s.store.get(f);
                let slide = &mut s.slides[f.index() as usize];
                if !jump && slide.published == Some(v) {
                    continue;
                }
                slide.published = Some(v);
                out.push((f, v, slide.live.is_some()));
            }
            (out, edited)
        };
        for (f, v, held) in &moved {
            let text: SharedString = f.spec().readout.format(*v).into();
            set_value(&g, *f, text.clone());
            if !held {
                set_readout(&g, *f, text);
                if jump {
                    self.place(*f, position(*f, *v));
                } else {
                    self.spring(*f, position(*f, *v));
                }
            }
        }
        let modified = {
            let s = self.state.borrow();
            Block::ALL.map(|b| (b, store::is_modified(s.store.settings(), b)))
        };
        for (b, on) in modified {
            set_modified(&g, b, on);
        }
        let reread = edited && {
            let s = self.state.borrow();
            s.open.is_some() && s.reading.as_ref().is_some_and(|r| *r != render::Reading::of(s.store.settings()))
        };
        if reread {
            self.redecode();
        }
        if edited || !moved.is_empty() || jump {
            self.state.borrow_mut().last_change = Some(Instant::now());
            self.schedule_write();
        }
        self.refresh_phone();
        self.publish_bar();
        self.publish_framing();
        self.publish_dedust();
        crate::editors::publish(self, jump);
        self.request_render();
    }

    /// Every property is read before any is set, so no Slint callback runs under the state's borrow.
    fn publish_bar(&self) {
        let Some(w) = self.window() else { return };
        self.publish_menus();
        let bar = w.global::<Bar>();
        let (has_photo, selected) = {
            let s = self.state.borrow();
            (s.open.is_some(), s.picks.selection.len())
        };
        let (live, titles) = (w.global::<Menus>().get_live(), w.global::<Menus>().get_titles());
        bar.set_has_photo(has_photo);
        bar.set_can_undo(live.undo);
        bar.set_can_redo(live.redo);
        bar.set_undo_title(titles.undo);
        bar.set_redo_title(titles.redo);
        bar.set_can_reset(live.reset);
        bar.set_targets(selected.max(usize::from(has_photo)) as i32);
        bar.set_export_title(crate::export::label::key(selected.max(usize::from(has_photo))).into());
        bar.set_editing(!w.global::<App>().get_gallery());
        self.publish_lab();
        self.publish_export_menu(selected, has_photo);
    }

    fn place(&self, f: Field, at: f32) {
        let i = f.index() as usize;
        self.arrivals[i].stop();
        {
            let mut s = self.state.borrow_mut();
            s.slides[i].arrival = None;
            s.slides[i].shown = at;
        }
        if let Some(w) = self.window() {
            set_fraction(&w.global::<StoreGlobal>(), f, at);
        }
    }

    /// `DS.Motion.settle`: a value arriving from elsewhere springs the knob; the canvas takes it at once.
    fn spring(self: &Rc<Self>, f: Field, to: f32) {
        let i = f.index() as usize;
        let from = {
            let mut s = self.state.borrow_mut();
            let from = s.slides[i].shown;
            s.slides[i].arrival = Some((from, to, Instant::now()));
            from
        };
        if from == to {
            return self.place(f, to);
        }
        let me = Rc::downgrade(self);
        self.arrivals[i].start(TimerMode::Repeated, FRAME_TICK, move || {
            let Some(me) = me.upgrade() else { return };
            let arrival = me.state.borrow().slides[i].arrival;
            let Some((from, to, at)) = arrival else { return me.arrivals[i].stop() };
            let t = at.elapsed().as_secs_f64();
            let spring = Spring::SETTLE;
            if t >= spring.settled_after() {
                me.place(f, to);
            } else {
                let at = from + (to - from) * spring.value(t) as f32;
                me.state.borrow_mut().slides[i].shown = at;
                if let Some(w) = me.window() {
                    set_fraction(&w.global::<StoreGlobal>(), f, at);
                }
            }
        });
    }

    // MARK: - Writing

    /// One notch: one store write, then what moved is published and the canvas asked again.
    fn write(self: &Rc<Self>, command: Command) {
        self.state.borrow_mut().store.apply(command);
        self.publish(false);
    }

    pub(crate) fn set(self: &Rc<Self>, f: Field, stored: f32) {
        if f == Field::Angle {
            return self.straighten(stored);
        }
        self.write(Command::Set { knob: f.knob(), value: stored });
    }

    // MARK: - Framing

    /// The Crop tab open on a photo in the editor: the canvas shows the frame uncropped.
    fn cropping(&self) -> bool {
        let Some(w) = self.window() else { return false };
        !w.global::<App>().get_gallery() && w.global::<PanelState>().get_tab() == 1 && self.state.borrow().open.is_some()
    }

    /// The open frame's raw size in full-resolution pixels; `Geometry` alone applies the quarter turns.
    fn image_size(&self) -> Option<(f64, f64)> {
        let s = self.state.borrow();
        let (_, n) = s.negative.as_ref()?;
        let (w, h) = n.size();
        Some((n.full_width, n.full_width * f64::from(h) / f64::from(w.max(1))))
    }

    fn write_geometry(self: &Rc<Self>, g: Geometry) {
        let mut next = self.state.borrow().store.settings().clone();
        if next.geometry == g {
            return;
        }
        next.geometry = g;
        self.write(Command::Write(Box::new(next)));
    }

    /// `GeometryPanel.angleBinding`: re-fitted from the gesture's own starting frame, so the
    /// intermediate angles leave no ratchet; a pause or a frame moved elsewhere starts a new one.
    fn straighten(self: &Rc<Self>, angle: f32) {
        let Some(image) = self.image_size() else { return };
        let fitted = {
            let mut s = self.state.borrow_mut();
            let current = s.store.settings().geometry.clone();
            let base = match &s.straightening {
                Some((base, written, at)) if at.elapsed() < STRAIGHTEN_IDLE && *written == current => base.clone(),
                _ => current,
            };
            let fitted = base.straightened(angle, image);
            s.straightening = Some((base, fitted.clone(), Instant::now()));
            fitted
        };
        self.write_geometry(fitted);
    }

    /// The image frame the overlays lie on, in points on the canvas: the upright oriented box while
    /// framing, the rendered picture otherwise, both centred where the viewport puts the render.
    fn image_frame(&self, framing: bool) -> Option<(f64, f64, f64, f64)> {
        let w = self.window()?;
        let backing = f64::from(w.window().scale_factor());
        let view = self.canvas_size()?;
        let image = self.image_size()?;
        let s = self.state.borrow();
        let rendered = s.full_size?;
        let full = if framing { s.store.settings().geometry.oriented_size(image) } else { rendered };
        let scale = s.viewport.effective_scale(rendered, view) / backing;
        let (cx, cy) = Viewport::centre((view.0 / backing, view.1 / backing), (s.viewport.offset.0 / backing, s.viewport.offset.1 / backing));
        let size = (full.0 * scale, full.1 * scale);
        Some((cx - size.0 / 2.0, cy - size.1 / 2.0, size.0, size.1))
    }

    /// The Crop tab and the canvas's frames from the open frame's geometry.
    fn publish_framing(&self) {
        let Some(w) = self.window() else { return };
        let cropping = self.cropping();
        let image = self.image_size();
        let crop_frame = self.image_frame(true);
        let picture_frame = self.image_frame(cropping);
        let (g, armed, board, window_board) = {
            let s = self.state.borrow();
            let board = s.store.editor_ref().is_some_and(|e| e.board.framing.is_some());
            (s.store.settings().geometry.clone(), s.window_armed, board, s.window_board)
        };
        w.global::<PanelState>().set_crop_first(g.crop == Fraction::UNIT && g.measure_window.is_none());
        let f = w.global::<crate::Framing>();
        let ratios = [oni_settings::frame::AspectRatio::ThreeTwo, oni_settings::frame::AspectRatio::FourThree, oni_settings::frame::AspectRatio::Square, oni_settings::frame::AspectRatio::SixSeven, oni_settings::frame::AspectRatio::FiveFour];
        f.set_ratio(g.ratio.and_then(|r| ratios.iter().position(|x| *x == r)).map_or(0, |i| i as i32 + 1));
        f.set_portrait(g.portrait);
        f.set_portrait_enabled(g.ratio.is_some_and(|r| r != oni_settings::frame::AspectRatio::Square));
        f.set_kept(image.map(|i| geom::pixel_label(g.cropped_pixel_size(i))).unwrap_or_default().into());
        f.set_modified(g != Geometry::reset_keeping(&g));
        f.set_mirrored(g.mirrored);
        f.set_mirrored_vertically(g.mirrored_vertically);
        f.set_can_paste(board);
        f.set_armed(armed);
        f.set_has_window(g.measure_window.is_some());
        f.set_can_paste_window(window_board.is_some());
        let c = w.global::<Canvas>();
        c.set_cropping(cropping && crop_frame.is_some());
        if let Some(frame) = crop_frame {
            let (x, y, wd, ht) = frames::shown(g.crop, frame);
            c.set_crop_x(x as f32);
            c.set_crop_y(y as f32);
            c.set_crop_width(wd as f32);
            c.set_crop_height(ht as f32);
            // `liveSize`: the kept pixels above the frame while a handle is held, inside the view.
            let held = self.state.borrow().frame_gesture.is_some_and(|(window, ..)| !window);
            let view = (f64::from(c.get_width()), f64::from(c.get_height()));
            let label = if held { image.map(|i| geom::pixel_label(g.cropped_pixel_size(i))).unwrap_or_default() } else { String::new() };
            c.set_crop_badge(label.into());
            c.set_crop_badge_x((x + wd / 2.0).clamp(BADGE_MARGIN, (view.0 - BADGE_MARGIN).max(BADGE_MARGIN)) as f32);
            c.set_crop_badge_y((y - BADGE_RISE).clamp(BADGE_MARGIN / 2.0, (view.1 - BADGE_MARGIN / 2.0).max(BADGE_MARGIN / 2.0)) as f32);
        }
        let window_frame = if cropping { crop_frame } else { picture_frame };
        c.set_window_shown(armed && window_frame.is_some());
        if let Some(frame) = window_frame {
            let (x, y, wd, ht) = frames::shown(frames::window_shown(&g, cropping), frame);
            c.set_window_x(x as f32);
            c.set_window_y(y as f32);
            c.set_window_width(wd as f32);
            c.set_window_height(ht as f32);
        }
    }

    /// A ratio or an orientation chosen: the frame cut to it in place and remembered for the next.
    fn choose_framing(self: &Rc<Self>, change: impl FnOnce(&mut Geometry)) {
        let Some(image) = self.image_size() else { return };
        let mut g = self.state.borrow().store.settings().geometry.clone();
        change(&mut g);
        g.apply_ratio(image, None, geom::Driving::Width);
        g.fit_crop_inside_image(image);
        {
            let mut s = self.state.borrow_mut();
            let path = s.open_path().map(str::to_string).unwrap_or_default();
            s.framing_memory = Remembered::new(g.ratio, g.portrait).map(|r| (r, path));
        }
        self.write_geometry(g);
    }

    fn framing_act(self: &Rc<Self>, action: &str) {
        let Some(image) = self.image_size() else { return };
        let mut g = self.state.borrow().store.settings().geometry.clone();
        match action {
            // Three quarters counter-clockwise is one clockwise, as the key says: the render turns the other way.
            "rotate" => g.rotate(3, image),
            "mirror" => g.mirror(),
            "mirror-vertically" => g.mirror_vertically(),
            "reset-crop" => g = Geometry::reset_keeping(&g),
            "copy-framing" => {
                if let Some(e) = self.state.borrow_mut().store.editor() {
                    e.copy_framing();
                }
                return self.publish_framing();
            }
            "paste-framing" | "paste-framing-to-roll" => return self.paste_framing(action == "paste-framing-to-roll"),
            "window-reset" => g.measure_window = None,
            "window-arm" => {
                let armed = {
                    let mut s = self.state.borrow_mut();
                    s.window_armed = !s.window_armed;
                    s.window_armed
                };
                if armed {
                    g.propose_measure_window(self.state.borrow().store.settings().sheet.is_some());
                }
            }
            "window-copy" => {
                let window = g.measure_window;
                self.state.borrow_mut().window_board = window;
                return self.publish_framing();
            }
            "window-paste" => {
                let Some(window) = self.state.borrow().window_board else { return };
                if g.measure_window == Some(window) {
                    return;
                }
                g.measure_window = Some(window);
            }
            _ => return,
        }
        self.write_geometry(g);
        self.publish_framing();
    }

    /// `pasteFraming()` on the selection, else the open photo, or `pasteFramingToRoll()`; one gallery step.
    fn paste_framing(self: &Rc<Self>, roll: bool) {
        if roll {
            self.paste_framing_to_roll();
        } else {
            let (targets, size) = (self.targets(), self.image_size());
            self.gesture(&targets, |store| store.paste_framing(&targets, size));
        }
        self.publish_framing();
    }

    /// The Crop tab opened: the framing the hand last chose is handed to this frame if chosen on another.
    fn tab_opened(self: &Rc<Self>, tab: i32) {
        if tab == 1 {
            let remembered = {
                let s = self.state.borrow();
                let open = s.open_path().map(str::to_string);
                s.framing_memory.as_ref().filter(|(_, on)| Some(on) != open.as_ref()).map(|(r, _)| *r)
            };
            if let (Some(r), Some(image)) = (remembered, self.image_size()) {
                let mut g = self.state.borrow().store.settings().geometry.clone();
                if (g.ratio, g.portrait) != (Some(r.ratio), r.portrait) {
                    g.adopt(r, image);
                    self.write_geometry(g);
                }
            }
        }
        self.publish_framing();
        self.request_render();
    }

    // MARK: - De-dust

    /// The De-dust pane open on a photo in the editor: a drag on the canvas paints.
    fn painting(&self) -> bool {
        let Some(w) = self.window() else { return false };
        !w.global::<App>().get_gallery() && w.global::<PanelState>().get_tab() == 4 && self.state.borrow().open.is_some()
    }

    /// `toggleCorrecting`: into the hidden pane, or out to the panel the hand was in, never the crop.
    fn toggle_correcting(self: &Rc<Self>) {
        let Some(w) = self.window() else { return };
        let panel = w.global::<PanelState>();
        let tab = panel.get_tab();
        let next = if tab == 4 {
            let last = self.state.borrow().last_panel;
            if last == 1 || last == 4 { 2 } else { last }
        } else {
            self.state.borrow_mut().last_panel = tab;
            4
        };
        panel.set_tab(next);
        w.global::<Bar>().set_dedust(next == 4);
        self.tab_opened(next);
        self.publish_dedust();
    }

    /// The brush, the corrections' rows and the canvas's marks from the open frame.
    fn publish_dedust(&self) {
        let Some(w) = self.window() else { return };
        let painting = self.painting();
        let frame = self.image_frame(false);
        let caches = crate::patches::caches();
        let (rows, marks, radius, stroke, rendered, traces, lines) = {
            let mut s = self.state.borrow_mut();
            let corrections = s.store.settings().corrections.clone();
            for c in &corrections {
                if !s.numbers.contains_key(&c.id) {
                    let n = s.next_number;
                    s.numbers.insert(c.id, n);
                    s.next_number += 1;
                }
            }
            let healing = s.healing.is_some() && s.healing.as_deref() == s.open_path();
            let rows: Vec<crate::CorrectionRow> = corrections
                .iter()
                .map(|c| crate::CorrectionRow {
                    label: s.numbers.get(&c.id).map_or("Correction".to_string(), |n| format!("Correction {n}")).into(),
                    pending: healing && !crate::patches::file(&caches, c.id).exists(),
                    fallback: s.fallback.contains(&c.id),
                })
                .collect();
            let rendered = s.full_size;
            let marks: Vec<(f64, f64, f64, f64, usize)> = match (frame, rendered) {
                (Some(f), Some(r)) => corrections
                    .iter()
                    .filter(|c| !c.stroke.is_empty())
                    .map(|c| {
                        // `boundingFraction`: the stroke's extent padded by its radius, in fractions.
                        let (rx, ry) = (c.radius / r.0, c.radius / r.1);
                        let (x0, x1) = c.stroke.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.x), b.max(p.x)));
                        let (y0, y1) = c.stroke.iter().fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p.y), b.max(p.y)));
                        let (x, y, wd, ht) = (x0 - rx, y0 - ry, x1 - x0 + 2.0 * rx, y1 - y0 + 2.0 * ry);
                        (f.0 + x * f.2, f.1 + y * f.3, wd * f.2, ht * f.3, s.numbers.get(&c.id).copied().unwrap_or(0))
                    })
                    .collect(),
                _ => Vec::new(),
            };
            // `correctionMarks`: the line from each stroke to its source, and the copied stroke itself.
            let traces: Vec<(Vec<(f64, f64)>, f64)> = corrections
                .iter()
                .filter_map(|c| {
                    let o = *s.sources.get(&c.id)?;
                    Some((c.stroke.iter().map(|p| (p.x + o.0, p.y + o.1)).collect(), c.radius))
                })
                .collect();
            let lines: Vec<((f64, f64), (f64, f64))> = corrections
                .iter()
                .filter_map(|c| {
                    let o = s.sources.get(&c.id)?;
                    let n = c.stroke.len().max(1) as f64;
                    let centre = (c.stroke.iter().map(|p| p.x).sum::<f64>() / n, c.stroke.iter().map(|p| p.y).sum::<f64>() / n);
                    Some((centre, (centre.0 + o.0, centre.1 + o.1)))
                })
                .collect();
            (rows, marks, s.brush, s.painting_stroke.clone(), rendered, traces, lines)
        };
        let d = w.global::<crate::Dedust>();
        let (low, high) = oni_settings::frame::Correction::RADIUS_RANGE;
        d.set_fraction(((f64::from(radius) - low) / (high - low)) as f32);
        let text: SharedString = format!("{radius:.0} px").into();
        d.set_readout(text.clone());
        d.set_value_text(text);
        d.set_rows(ModelRc::new(VecModel::from(rows)));
        let c = w.global::<Canvas>();
        c.set_painting(painting);
        // `strokeLineWidth`: the radius in full-resolution pixels as a width on the canvas.
        let width = match (frame, rendered) {
            (Some(f), Some(r)) => f64::from(radius) * 2.0 * f.2 / r.0.max(1.0),
            _ => 0.0,
        };
        c.set_brush_diameter(width as f32);
        c.set_stroke_width(width as f32);
        let commands = match frame {
            Some(f) if stroke.len() > 1 => stroke
                .iter()
                .enumerate()
                .map(|(i, p)| format!("{} {:.2} {:.2}", if i == 0 { "M" } else { "L" }, f.0 + p.0 * f.2, f.1 + p.1 * f.3))
                .collect::<Vec<_>>()
                .join(" "),
            _ => String::new(),
        };
        c.set_stroke(commands.into());
        let marks: Vec<Mark> = marks
            .into_iter()
            .map(|(x, y, wd, ht, number)| Mark { x: x as f32, y: y as f32, width: wd as f32, height: ht as f32, number: number as i32 })
            .collect();
        c.set_marks(ModelRc::new(VecModel::from(marks)));
        let path = |points: &[(f64, f64)], f: (f64, f64, f64, f64)| {
            let mut out = String::new();
            for (i, p) in points.iter().enumerate() {
                out.push_str(&format!("{} {:.2} {:.2} ", if i == 0 { "M" } else { "L" }, f.0 + p.0 * f.2, f.1 + p.1 * f.3));
            }
            if points.len() == 1 {
                out.push_str(&format!("L {:.2} {:.2}", f.0 + points[0].0 * f.2, f.1 + points[0].1 * f.3));
            }
            out
        };
        let (shapes, line_commands) = match (frame, rendered) {
            (Some(f), Some(r)) => (
                traces
                    .iter()
                    .map(|(points, radius)| (path(points, f).into(), (radius * 2.0 * f.2 / r.0.max(1.0)) as f32))
                    .collect::<Vec<(SharedString, f32)>>(),
                lines.iter().map(|(a, b)| path(&[*a, *b], f)).collect::<Vec<_>>().join(" "),
            ),
            _ => (Vec::new(), String::new()),
        };
        c.set_source_shapes(ModelRc::new(VecModel::from(shapes)));
        c.set_source_lines(line_commands.into());
        c.set_source_line_width(1.5);
    }

    /// A point of the canvas in fractions of the rendered frame, when it falls on the picture.
    fn on_picture(&self, x: f32, y: f32) -> Option<(f64, f64)> {
        let f = self.image_frame(false)?;
        let (fx, fy) = ((f64::from(x) - f.0) / f.2, (f64::from(y) - f.1) / f.3);
        ((0.0..=1.0).contains(&fx) && (0.0..=1.0).contains(&fy)).then_some((fx, fy))
    }

    fn paint_hover(&self, x: f32, y: f32, hover: bool) {
        let Some(w) = self.window() else { return };
        let c = w.global::<Canvas>();
        c.set_brush_shown(hover && self.on_picture(x, y).is_some());
        c.set_brush_x(x);
        c.set_brush_y(y);
    }

    fn paint_at(&self, x: f32, y: f32, start: bool) {
        let point = self.on_picture(x, y);
        {
            let mut s = self.state.borrow_mut();
            if start {
                s.painting_stroke.clear();
            }
            if let Some(p) = point {
                s.painting_stroke.push(p);
            }
        }
        self.paint_hover(x, y, true);
        self.publish_dedust();
    }

    /// `appendCorrection`: a stroke of two points at least becomes a correction, its patch generated next.
    fn paint_end(self: &Rc<Self>) {
        let (stroke, radius) = {
            let mut s = self.state.borrow_mut();
            (std::mem::take(&mut s.painting_stroke), s.brush)
        };
        if stroke.len() > 1 {
            let correction = oni_settings::frame::Correction {
                id: oni_settings::id::Id::new(),
                stroke: stroke.iter().map(|&(x, y)| oni_settings::cg::Point { x, y }).collect(),
                radius: f64::from(radius),
            };
            let mut next = self.state.borrow().store.settings().clone();
            next.corrections.push(correction);
            self.write(Command::Write(Box::new(next)));
        }
        self.publish_dedust();
    }

    /// `deleteCorrection`: gone from the settings, its patch from the cache and the engine.
    fn remove_correction(self: &Rc<Self>, row: usize) {
        let mut next = self.state.borrow().store.settings().clone();
        if row >= next.corrections.len() {
            return;
        }
        let gone = next.corrections.remove(row);
        let _ = std::fs::remove_file(crate::patches::file(&crate::patches::caches(), gone.id));
        if let Ok(mut e) = self.engine.lock() {
            e.forget_patch(gone.id);
        }
        self.state.borrow_mut().fallback.remove(&gone.id);
        self.write(Command::Write(Box::new(next)));
    }

    /// The brush slider, held as `DSSlider` holds a value: a double press resets it to the default.
    fn brush_press(&self) -> bool {
        let mut s = self.state.borrow_mut();
        let double = s.brush_pressed_at.is_some_and(|t| t.elapsed() < system::double_click());
        s.brush_pressed_at = Some(Instant::now());
        if double {
            s.brush = oni_settings::frame::Correction::DEFAULT_RADIUS as f32;
            s.brush_start = None;
            return true;
        }
        let (low, high) = oni_settings::frame::Correction::RADIUS_RANGE;
        s.brush_start = Some(((f64::from(s.brush) - low) / (high - low)) as f32);
        false
    }

    fn brush_drag(&self, translation: f32, span: f32, fine: bool) {
        {
            let mut s = self.state.borrow_mut();
            let Some(start) = s.brush_start else { return };
            let f = track::clamped(track::asked(start, translation, span, fine));
            let (low, high) = oni_settings::frame::Correction::RADIUS_RANGE;
            s.brush = (low + f64::from(f) * (high - low)) as f32;
        }
        self.publish_dedust();
    }

    /// A frame on the canvas held, moved and let go: one undo step, constrained on every event.
    fn frame_grabbed(self: &Rc<Self>, window: bool, handle: i32) {
        let Some(handle) = Handle::from_index(handle) else { return };
        let cropping = self.cropping();
        let Some(frame) = self.image_frame(cropping || !window) else { return };
        let mut s = self.state.borrow_mut();
        let g = s.store.settings().geometry.clone();
        let from = if window { frames::window_shown(&g, cropping) } else { g.crop };
        s.frame_gesture = Some((window, handle, from, (frame.2, frame.3)));
        s.store.hold();
    }

    fn frame_dragged(self: &Rc<Self>, dx: f32, dy: f32) {
        let Some(image) = self.image_size() else { return };
        let cropping = self.cropping();
        let Some((window, handle, from, frame)) = self.state.borrow().frame_gesture else { return };
        let proposed = frames::apply(handle, from, frame, (f64::from(dx), f64::from(dy)));
        let g = self.state.borrow().store.settings().geometry.clone();
        let next = if window {
            Geometry { measure_window: Some(frames::window_stored(&g, frames::bounded(proposed), cropping)), ..g }
        } else {
            frames::constrained_crop(&g, handle, proposed, image)
        };
        self.write_geometry(next);
        self.publish_framing();
    }

    fn frame_released(self: &Rc<Self>) {
        let held = self.state.borrow_mut().frame_gesture.take().is_some();
        if held {
            self.state.borrow_mut().store.release();
            self.publish(false);
        }
    }

    // MARK: - Rendering

    /// Lays the open frame's current settings in the render slot, when they differ from the last asked.
    fn request_render(self: &Rc<Self>) {
        let size = self.canvas_size();
        let cropping = self.cropping();
        let (job, build, heal) = {
            let mut s = self.state.borrow_mut();
            let Some(path) = s.open_path().map(str::to_string) else { return };
            let revision = s.store.revision();
            let moved_at = s.moved_at.take();
            let settled = s.navigated_at.is_none_or(|t| t.elapsed() >= viewport::SETTLE);
            let full = s.full.as_ref().filter(|(p, _)| *p == path).map(|(_, f)| f.clone());
            let view_key = size.map(|size| (s.viewport, size, settled, full.is_some(), s.split.is_some(), cropping));
            let moved = revision != s.requested || s.rendered == 0;
            if !moved && view_key == s.requested_view {
                return;
            }
            // `NegativeView.live`: while framing the crop is removed, so a crop drag keeps the pixels.
            let mut live = s.store.settings().clone();
            if let Some(previewed) = s.preset_preview(&live) {
                live = previewed;
            }
            if cropping {
                live.geometry = live.geometry.without_crop();
            }
            if s.rendered != 0 && view_key == s.requested_view && s.requested_live.as_ref() == Some(&live) {
                s.requested = revision;
                return;
            }
            let Some(negative) = s.negative.as_ref().filter(|(p, _)| *p == path).map(|(_, n)| n.clone()) else { return };
            s.requested = revision;
            s.requested_view = view_key;
            s.requested_live = Some(live.clone());
            // Until the frame's first picture is shown, a render is the opening one, timed from the open.
            let (kind, asked) = if s.rendered == 0 {
                (Kind::Open, s.opened_at)
            } else if moved {
                (Kind::Notch, moved_at.unwrap_or_else(Instant::now))
            } else {
                (Kind::View, Instant::now())
            };
            // Full resolution is built once the view settles past the preview's ratio, never mid-gesture.
            let wants_full = match (size, s.full_size) {
                (Some(size), Some(f)) => settled && s.viewport.needs_full_resolution(negative.size().0 as f64 / negative.full_width, f, size, viewport::NOMINAL_BUDGET),
                _ => false,
            };
            let build = (wants_full && full.is_none() && s.building_full.as_deref() != Some(path.as_str())).then(|| {
                s.building_full = Some(path.clone());
                (path.clone(), render::Reading::of(s.store.settings()))
            });
            let view = size.map(|size| View { viewport: s.viewport, size, full, settled });
            // The cache is purged at every quit, so each correction's patch is generated as its frame opens.
            let caches = crate::patches::caches();
            let stored = s.store.settings().clone();
            let unhealed = stored.corrections.iter().any(|c| !c.stroke.is_empty() && !crate::patches::file(&caches, c.id).exists());
            let heal = (unhealed && s.healing.as_deref() != Some(path.as_str())).then(|| {
                s.healing = Some(path.clone());
                (path.clone(), stored)
            });
            // `NegativeView.referenceSettings`: the bar's reset frame, the dust still healed.
            let reference = s.split.map(|_| {
                let mut reference = oni_core::parameters::imported(&live);
                reference.corrections = live.corrections.clone();
                reference
            });
            // A notch is the hand at work: the input histograms read the small copy until it stops.
            let held = kind == Kind::Notch;
            (Job { revision, path: PathBuf::from(path), negative, settings: live, kind, asked, view, reference, held }, build, heal)
        };
        self.worker.submit(job);
        if let Some((path, converted)) = build {
            self.build_full(path, converted);
        }
        if let Some((path, settings)) = heal {
            self.heal(path, settings);
            self.publish_dedust();
        }
    }

    /// `ensureGenerated` for each correction of the open frame that has no patch yet, one after the
    /// other on the full-resolution frame, so each source holds the patches before it healed.
    fn heal(self: &Rc<Self>, path: String, settings: oni_settings::PipelineSettings) {
        let engine = self.engine.clone();
        std::thread::spawn(move || {
            let caches = crate::patches::caches();
            let made = (|| -> Result<Vec<(oni_settings::id::Id, bool, (f64, f64))>, String> {
                let decoded = render::decode_full(Path::new(&path), render::Reading::of(&settings))?;
                let full = engine.lock()?.full(decoded)?;
                let mut made = Vec::new();
                for c in &settings.corrections {
                    if c.stroke.is_empty() || crate::patches::file(&caches, c.id).exists() {
                        continue;
                    }
                    let (region, size) = engine.lock()?.search_region(&full, &settings, c)?;
                    let patch = crate::heal::generate(&region, c, size.0, size.1);
                    crate::patches::write(&caches, c.id, patch.width as u32, patch.height as u32, &patch.pixels)?;
                    engine.lock()?.forget_patch(c.id);
                    made.push((c.id, patch.fallback, patch.offset));
                }
                Ok(made)
            })();
            with_shell(move |c| {
                c.state.borrow_mut().healing = None;
                match made {
                    Ok(m) if m.is_empty() => {}
                    Ok(m) => {
                        let n = m.len();
                        for (id, fell_back, offset) in m {
                            let mut s = c.state.borrow_mut();
                            if fell_back {
                                s.fallback.insert(id);
                            }
                            s.sources.insert(id, offset);
                        }
                        timing::line(&format!("{}: {n} dust patches generated", stem(&path)));
                        if c.state.borrow().open_path() == Some(path.as_str()) {
                            c.state.borrow_mut().requested_view = None;
                            c.request_render();
                        }
                    }
                    Err(e) => timing::line(&format!("{}: dust patches not generated: {e}", stem(&path))),
                }
                c.publish_dedust();
            });
        });
    }

    /// The canvas in drawable pixels, once laid out.
    fn canvas_size(&self) -> Option<(f64, f64)> {
        let w = self.window()?;
        let (c, scale) = (w.global::<Canvas>(), f64::from(w.window().scale_factor()));
        let size = (f64::from(c.get_width()) * scale, f64::from(c.get_height()) * scale);
        (size.0 >= 1.0 && size.1 >= 1.0).then_some(size)
    }

    /// The open frame decoded again at full resolution off this thread, then shown where the view wants it.
    fn build_full(self: &Rc<Self>, path: String, converted: render::Reading) {
        let engine = self.engine.clone();
        std::thread::spawn(move || {
            let built = render::decode_full(Path::new(&path), converted).and_then(|d| engine.lock()?.full(d));
            with_shell(move |c| {
                let open = c.state.borrow().open_path() == Some(path.as_str());
                {
                    let mut s = c.state.borrow_mut();
                    s.building_full = None;
                    match built {
                        Ok(full) if open => s.full = Some((path, full)),
                        Ok(_) => {}
                        Err(e) => timing::line(&format!("full resolution not built: {e}")),
                    }
                }
                if open {
                    c.request_render();
                }
            });
        });
    }

    /// Applies a navigation at once on the preview, then asks again once it has settled.
    fn navigate(self: &Rc<Self>, change: impl FnOnce(&mut Viewport, (f64, f64), (f64, f64))) {
        let Some(size) = self.canvas_size() else { return };
        {
            let mut s = self.state.borrow_mut();
            let Some(full) = s.full_size else { return };
            let before = s.viewport;
            change(&mut s.viewport, full, size);
            if s.viewport == before {
                return;
            }
            s.navigated_at = Some(Instant::now());
        }
        self.request_render();
        let me = Rc::downgrade(self);
        // The timer marks the view settled itself: it can fire a hair before the debounce reads as elapsed.
        self.settle.start(TimerMode::SingleShot, viewport::SETTLE, move || {
            if let Some(me) = me.upgrade() {
                me.state.borrow_mut().navigated_at = None;
                me.request_render();
            }
        });
    }

    /// The bar's − / +: the zoom ladder in the editor.
    fn step_zoom(self: &Rc<Self>, by: i32) {
        let rung = {
            let s = self.state.borrow();
            let (Some(full), Some(size)) = (s.full_size, self.canvas_size()) else { return };
            Viewport::rung(s.viewport.effective_scale(full, size), s.viewport.is_fitted(), by)
        };
        if let Some(rung) = rung {
            self.navigate(|v, full, size| v.step(rung, full, size));
        }
    }

    /// A render arrived: stale ones are dropped, the GPU path encodes its texture into the canvas,
    /// the software renderer shows the bytes encoded on the render thread.
    fn deliver(self: &Rc<Self>, done: Result<Done, String>) {
        let Some(w) = self.window() else { return };
        let done = match done {
            Ok(d) => d,
            Err(e) => {
                // The canvas never keeps another photo's picture under this one's name.
                w.global::<Canvas>().set_frame(Image::default());
                w.global::<Canvas>().set_busy(false);
                w.global::<Plots>().set_output(Image::default());
                self.state.borrow_mut().histogram_dirty = false;
                self.say(&format!("Not rendered: {e}"));
                return;
            }
        };
        {
            let s = self.state.borrow();
            let current = s.open_path().map(PathBuf::from);
            if current.as_ref() != Some(&done.path) || done.revision < s.rendered {
                return;
            }
        }
        let shown = Shown { kind: done.kind, asked: done.asked, pipeline: done.pipeline, frame: stem(&done.path.to_string_lossy()) };
        let (width, height) = (done.width, done.height);
        self.lay(done.placed, (width, height));
        if let Some((before, placed)) = done.before {
            self.encode_before(before, placed);
        }
        let on_gpu = {
            let mut s = self.state.borrow_mut();
            s.rendered = done.revision.max(1);
            s.renders += 1;
            if let Some(h) = done.histogram {
                s.histogram = Some(h);
                s.histogram_dirty = true;
            }
            if let Some(i) = done.inputs {
                s.inputs = Some(i);
                s.inputs_drawn = None;
            }
            s.gpu.is_some()
        };
        self.publish_inputs();
        if on_gpu {
            let whole = done.placed.is_none_or(|p| p.whole);
            self.encode(done.picture, shown, whole);
            w.window().request_redraw();
        } else {
            if let Picture::Pixels { rgba8, .. } = done.picture {
                w.global::<Canvas>().set_frame(image(&rgba8, width, height));
            }
            self.shown(shown);
        }
        let status = {
            let s = self.state.borrow();
            let name = s.open_path().map(stem).unwrap_or_default();
            match s.placed {
                Some(p) => format!("{name} · {:.0} × {:.0}", p.full.0, p.full.1),
                None => format!("{name} · {width} × {height}"),
            }
        };
        w.global::<Canvas>().set_busy(false);
        w.global::<Canvas>().set_status(status.into());
    }

    /// Lays the render where the viewport puts it; a render with no place fills the canvas, fitted.
    fn lay(&self, placed: Option<Placed>, size: (usize, usize)) {
        let Some(w) = self.window() else { return };
        let scale = f64::from(w.window().scale_factor());
        let rect = match (placed, self.canvas_size()) {
            (Some(p), _) => (p.x, p.y, p.width, p.height),
            (None, Some(view)) => Viewport::default().placement((size.0 as f64, size.1 as f64), view).image,
            (None, None) => return,
        };
        {
            let mut s = self.state.borrow_mut();
            s.placed = placed;
            if let Some(p) = placed {
                s.full_size = Some(p.full);
            }
        }
        let c = w.global::<Canvas>();
        c.set_frame_x((rect.0 / scale) as f32);
        c.set_frame_y((rect.1 / scale) as f32);
        c.set_frame_width((rect.2 / scale) as f32);
        c.set_frame_height((rect.3 / scale) as f32);
        let bubble = {
            let s = self.state.borrow();
            match (placed, self.canvas_size()) {
                (Some(p), Some(size)) => {
                    let current = s.viewport.effective_scale(p.full, size);
                    let fitted = s.viewport.is_fitted();
                    Some((viewport::zoom_label(current, p.at_full_resolution), Viewport::rung(current, fitted, -1).is_some(), Viewport::rung(current, fitted, 1).is_some()))
                }
                _ => None,
            }
        };
        let (label, out, inward) = bubble.unwrap_or_default();
        c.set_zoom_label(label.into());
        c.set_can_zoom_out(out);
        c.set_can_zoom_in(inward);
        self.publish_framing();
        self.publish_dedust();
    }

    /// Compare's before into its own texture, laid where its view puts it.
    fn encode_before(&self, before: oni_graph::Source, placed: Option<Placed>) {
        let Some(w) = self.window() else { return };
        let canvas = self.canvas_size();
        let (imported, rect) = {
            let mut s = self.state.borrow_mut();
            let screen = s.screen;
            let Some(g) = s.gpu.as_mut() else { return };
            let Some(view) = before.view().cloned() else { return };
            let e = before.extent();
            let reuse = g.before.as_ref().filter(|(t, _)| t.width() == e.w && t.height() == e.h).map(|(t, _)| t.clone());
            let (target, imported) = match reuse {
                Some(t) => (t, None),
                None => {
                    let t = display::Gpu::target(&g.device, e.w, e.h);
                    (t.clone(), Image::try_from(t).ok())
                }
            };
            let k = zebra(&s, Params::new(screen, e.w, e.h, s.gpu.as_ref().map_or(0, |g| g.seed)), placed, e.w, canvas);
            let Some(g) = s.gpu.as_mut() else { return };
            g.kernel.run(&g.device, &g.queue, &view, &k, &target);
            if g.before.as_ref().is_none_or(|(_, kept)| kept.id() != before.id()) {
                s.befores += 1;
            }
            s.before_placed = placed;
            let Some(g) = s.gpu.as_mut() else { return };
            g.before = Some((target, before));
            (imported, placed)
        };
        let c = w.global::<Canvas>();
        if let Some(image) = imported {
            c.set_before(image);
        }
        if let Some(p) = rect {
            let scale = f64::from(w.window().scale_factor());
            c.set_before_x((p.x / scale) as f32);
            c.set_before_y((p.y / scale) as f32);
            c.set_before_width((p.width / scale) as f32);
            c.set_before_height((p.height / scale) as f32);
        }
    }

    /// `ScreenGamut`'s stripes on and off; while on the canvas is encoded again at their pace,
    /// from the render it holds, so the stripes move and the pipeline never runs for them.
    fn toggle_screen_gamut(self: &Rc<Self>) {
        let on = {
            let mut s = self.state.borrow_mut();
            s.screen_gamut = !s.screen_gamut;
            s.screen_gamut
        };
        if on {
            let me = Rc::downgrade(self);
            let pace = Duration::from_secs_f64(1.0 / f64::from(oni_colour::gamut::FRAMES_PER_SECOND));
            self.stripes.start(TimerMode::Repeated, pace, move || {
                if let Some(me) = me.upgrade() {
                    me.restripe();
                }
            });
        } else {
            self.stripes.stop();
        }
        self.restripe();
    }

    /// The canvas, and Compare's before, encoded again from the renders they hold.
    fn restripe(&self) {
        let Some(w) = self.window() else { return };
        let canvas = self.canvas_size();
        {
            let s = self.state.borrow();
            let screen = s.screen;
            let Some(g) = s.gpu.as_ref() else { return };
            if let (Some(Picture::Resident(source)), Some(target), Some(view)) = (&g.current, &g.target, g.current.as_ref().and_then(picture_view)) {
                let e = source.extent();
                let k = zebra(&s, Params::new(screen, e.w, e.h, g.seed), s.placed, e.w, canvas);
                g.kernel.run(&g.device, &g.queue, &view, &k, target);
            }
            if let Some((target, before)) = &g.before
                && let Some(view) = before.view()
            {
                let e = before.extent();
                let k = zebra(&s, Params::new(screen, e.w, e.h, g.seed), s.before_placed, e.w, canvas);
                g.kernel.run(&g.device, &g.queue, view, &k, target);
            }
        }
        w.window().request_redraw();
    }

    /// Compare Before/After on and off, the split at half the canvas.
    fn toggle_compare(self: &Rc<Self>) {
        let split = {
            let mut s = self.state.borrow_mut();
            s.split = if s.split.is_some() { None } else { Some(0.5) };
            s.split
        };
        if let Some(w) = self.window() {
            w.global::<Canvas>().set_split(split.unwrap_or(-1.0));
        }
        self.request_render();
    }

    /// On a wgpu renderer the display kernel writes the canvas texture, which the next frame draws.
    fn encode(&self, picture: Picture, shown: Shown, whole: bool) {
        let Some(w) = self.window() else { return };
        let canvas = self.canvas_size();
        let image = {
            let mut s = self.state.borrow_mut();
            let screen = s.screen;
            let Some(g) = s.gpu.as_mut() else { return };
            let uploaded;
            let (source, wd, ht) = match &picture {
                Picture::Resident(source) => {
                    let e = source.extent();
                    (source.view().expect("a resident render is one texture").clone(), e.w, e.h)
                }
                Picture::Pixels { linear, .. } => {
                    let (w, h) = (linear.width as u32, linear.height as u32);
                    uploaded = display::upload(&g.device, &g.queue, &linear.pixels, w, h);
                    (uploaded.create_view(&Default::default()), w, h)
                }
            };
            // The texture is imported once per size; a new render only rewrites its texels.
            let (target, imported) = match &g.target {
                Some(t) if t.width() == wd && t.height() == ht => (t.clone(), None),
                _ => {
                    let t = display::Gpu::target(&g.device, wd, ht);
                    g.target = Some(t.clone());
                    (t.clone(), Image::try_from(t).ok())
                }
            };
            g.seed = g.seed.wrapping_add(1);
            let seed = g.seed;
            let k = zebra(&s, Params::new(screen, wd, ht, seed), s.placed, wd, canvas);
            let Some(g) = s.gpu.as_mut() else { return };
            g.kernel.run(&g.device, &g.queue, &source, &k, &target);
            g.current = Some(picture);
            // A region is not the picture: its histogram would describe what the zoom shows.
            g.uncounted = whole;
            (imported, shown)
        };
        if let (Some(image), _) = &image {
            w.global::<Canvas>().set_frame(image.clone());
        }
        self.shown(image.1);
    }

    /// A render reached the screen: its line, and a notch's place in the centiles.
    fn shown(&self, shown: Shown) {
        let end_to_end = shown.asked.elapsed();
        timing::render(shown.kind, &shown.frame, shown.pipeline, end_to_end);
        if shown.kind == Kind::View {
            self.state.borrow_mut().views.record(end_to_end, Instant::now());
        }
        if shown.kind == Kind::Notch {
            let report = {
                let mut s = self.state.borrow_mut();
                s.notches.record(end_to_end, Instant::now());
                s.notches.due()
            };
            if let Some(report) = report {
                timing::line(&report);
            }
        }
    }

    /// The histogram and the load counter, on their own pace so a notch touches neither.
    fn pulse(self: &Rc<Self>) {
        let Some(w) = self.window() else { return };
        let (histogram, loading) = {
            let mut s = self.state.borrow_mut();
            if let Some(h) = s.gpu.as_mut().and_then(Gpu::landed) {
                s.histogram = Some(h);
                s.histogram_dirty = true;
            }
            let histogram = if s.histogram_dirty {
                s.histogram_dirty = false;
                s.histogram.clone()
            } else {
                None
            };
            (histogram, s.load_bar())
        };
        if let Some(h) = histogram {
            let (pw, ph) = HISTOGRAM;
            w.global::<Plots>().set_output(premultiplied(&h.raster(pw, ph), pw, ph));
        }
        if w.global::<App>().get_gallery() {
            w.global::<Library>().set_loading(loading.unwrap_or_default().into());
        }
    }

    /// The levels' and the manual curves' plots over their input distributions, each with its own
    /// channel in focus; drawn again only when a reading lands or a channel tab moves.
    pub(crate) fn publish_inputs(&self) {
        let Some(w) = self.window() else { return };
        let focus = {
            let g = self.editors.gestures.borrow();
            (g.levels_channel().histogram_channel(), g.curve_channel().histogram_channel())
        };
        let inputs = {
            let mut s = self.state.borrow_mut();
            if s.inputs_drawn == Some(focus) {
                return;
            }
            s.inputs_drawn = Some(focus);
            s.inputs.clone()
        };
        let (pw, ph) = HISTOGRAM;
        let plots = w.global::<Plots>();
        match inputs {
            Some(i) => {
                plots.set_levels(premultiplied(&render::InputHistograms::raster(&i.levels, focus.0, pw, ph), pw, ph));
                plots.set_curves(premultiplied(&render::InputHistograms::raster(&i.curves, focus.1, pw, ph), pw, ph));
            }
            None => {
                plots.set_levels(Image::default());
                plots.set_curves(Image::default());
            }
        }
    }

    /// The deferred write, `WRITE_DELAY` after the last change; a hand still on a slider puts it off
    /// until its release, which asks again.
    fn schedule_write(self: &Rc<Self>) {
        let me = Rc::downgrade(self);
        self.writer.start(TimerMode::SingleShot, WRITE_DELAY, move || {
            let Some(me) = me.upgrade() else { return };
            let failure = {
                let mut s = me.state.borrow_mut();
                if s.slides.iter().any(|sl| sl.live.is_some()) {
                    return;
                }
                s.last_change = None;
                s.store.flush()
            };
            match failure {
                Some(message) => me.say(&message),
                None => me.schedule_redraw(),
            }
        });
    }

    /// Writes the open frame's sidecar now when the disk differs.
    pub(crate) fn flush(&self) {
        let failure = self.state.borrow_mut().store.flush();
        if let Some(message) = failure {
            self.say(&message);
        }
    }

    fn say(&self, text: &str) {
        self.state.borrow_mut().status = text.to_string();
        if let Some(w) = self.window() {
            w.global::<Canvas>().set_status(text.into());
        }
    }

    // MARK: - The archive

    /// `add(_:)`: a folder opens; a file opens its folder, then itself in the editor once found.
    fn add(self: &Rc<Self>, path: &Path) {
        let file = !path.is_dir();
        let folder = if file { path.parent().unwrap_or(path) } else { path };
        let opening = (file && oni_library::library::is_importable(path)).then(|| oni_library::paths::canonical(path));
        self.open_folder(folder);
        self.state.borrow_mut().opening = opening;
    }

    /// The launch's folder: `openedFolder` if it still exists, else the first root of `roots.json`.
    fn open_remembered(self: &Rc<Self>) -> bool {
        let (remembered, support) = {
            let s = self.state.borrow();
            (s.preferences.string(preferences::OPENED_FOLDER).map(PathBuf::from), s.support.clone())
        };
        let roots = || oni_library::roots::Roots::load(&support).current().map(PathBuf::from);
        let folder = remembered.filter(|p| p.is_dir()).or_else(roots);
        match folder {
            Some(folder) => {
                self.open_folder(&folder);
                true
            }
            None => false,
        }
    }

    /// `open()`: the system's folder picker, then `add`; a cancel leaves everything as it was.
    fn pick(self: &Rc<Self>) {
        // Taken out first: a modal panel runs a nested event loop, which may reach the state.
        let picker = std::mem::replace(&mut self.state.borrow_mut().picker, Box::new(|| None));
        let chosen = picker();
        self.state.borrow_mut().picker = picker;
        if let Some(folder) = chosen {
            self.add(&folder);
        } else if !system::HAS_PICKER {
            self.say("This system has no folder picker: pass the folder on the command line");
        }
    }

    /// `openFolder`: the gallery on the folder, read through the index behind, its thumbnails
    /// drawn once the read is whole; the editor keeps what it holds.
    fn open_folder(self: &Rc<Self>, folder: &Path) {
        let folder = oni_library::paths::canonical(folder);
        let (generation, index_path) = {
            let mut s = self.state.borrow_mut();
            s.org.trash_open = false;
            s.trash_picks = Default::default();
            s.opened = Some(folder.clone());
            let _ = s.preferences.set(preferences::OPENED_FOLDER, serde_json::Value::String(folder.clone()));
            s.opening = None;
            s.read_done = false;
            s.creating = (0, 0);
            s.painted = (0, 0);
            s.painter.0.lock().expect("painter").pass = None;
            (s.generation.fetch_add(1, Ordering::Relaxed) + 1, s.index_path.clone())
        };
        self.read_folder(generation, folder, index_path);
    }

    /// `rescan`: the opened folder read again, the index trimmed to what the disk holds.
    fn read_again(self: &Rc<Self>) {
        let (folder, index_path, generation) = {
            let mut s = self.state.borrow_mut();
            let Some(folder) = s.opened.clone() else { return };
            s.read_done = false;
            (folder, s.index_path.clone(), s.generation.load(Ordering::Relaxed))
        };
        self.read_folder(generation, folder, index_path);
    }

    /// The read on its own thread and its own connection; each landing comes back to this thread.
    fn read_folder(self: &Rc<Self>, generation: u64, folder: String, index_path: PathBuf) {
        let current = self.state.borrow().generation.clone();
        std::thread::Builder::new()
            .name("library".into())
            .spawn(move || {
                let Some(index) = LibraryIndex::open(&index_path) else {
                    return with_shell(|c| c.say("The library index cannot be opened"));
                };
                let alive = || current.load(Ordering::Relaxed) == generation;
                library::read_rows(&folder, &index, &alive, &mut |rows, done, survey| {
                    let survey = survey.clone();
                    with_shell(move |c| c.take_rows(generation, rows, done, survey));
                });
            })
            .expect("library thread");
    }

    /// `takeRows` then `layGallery`: the rows laid, the selection and the open photo kept by path.
    fn take_rows(self: &Rc<Self>, generation: u64, rows: Vec<oni_library::Entry>, done: bool, survey: Survey) {
        {
            let mut s = self.state.borrow_mut();
            if s.generation.load(Ordering::Relaxed) != generation {
                return;
            }
            s.fingerprints = rows.iter().map(|e| (e.path.clone(), e.fingerprint.clone())).collect();
            s.rows = rows;
            s.survey = survey;
            s.read_done = done;
        }
        self.lay_archive();
        if done {
            let order = self.state.borrow().order.clone();
            self.offer_neutral_decode_notice(&order);
            self.fill();
            let opening = self.state.borrow_mut().opening.take();
            let found = opening.and_then(|o| {
                let s = self.state.borrow();
                if o.is_empty() { s.order.first().cloned() } else { s.order.iter().find(|p| **p == o).cloned() }
            });
            if let Some(path) = found {
                self.open_in_editor(&path);
            }
        }
    }

    /// The archive cut again from the rows held, under the filter; what the hand unfolded stays so.
    fn lay_archive(self: &Rc<Self>) {
        let (title, stock) = {
            let mut s = self.state.borrow_mut();
            let Some(folder) = s.opened.clone() else { return };
            let mut archive = Archive::from_rows(&folder, &s.rows, &s.survey, s.filter);
            if let Some(old) = &s.archive {
                let folded: HashSet<_> = old.nodes.iter().map(oni_library::archive_tree::Node::id).filter(|id| !old.expanded.contains(id)).collect();
                archive.expanded.retain(|id| !folded.contains(id));
            }
            let every: Vec<String> = s.rows.iter().map(|e| e.path.clone()).collect();
            s.store.set_archive(every);
            let first = archive.groups.first().map(|g| (g.title.clone(), g.stock.clone())).unwrap_or_default();
            s.archive = Some(archive);
            first
        };
        if let Some(w) = self.window() {
            let lib = w.global::<Library>();
            lib.set_roll_title(title.into());
            lib.set_stock(stock.into());
        }
        self.read_trashed();
        self.lay_grid();
        self.publish_tree();
        self.publish_bar();
    }

    /// `fillOpened`: the pass over the opened folder's rows, which draws those with no file.
    fn fill(&self) {
        let s = self.state.borrow();
        let generation = s.generation.load(Ordering::Relaxed);
        let rows: Vec<(String, String)> = s
            .rows
            .iter()
            .filter(|e| !e.fingerprint.starts_with(thumbnails::PENDING))
            .map(|e| (e.path.clone(), e.fingerprint.clone()))
            .collect();
        let (lock, wake) = &*s.painter;
        lock.lock().expect("painter").pass = Some((generation, rows));
        wake.notify_one();
    }

    /// `invalidate`: the photo's redraw, queued once and drawn before the pass's next photo.
    fn redraw(&self, path: &str) {
        let mut s = self.state.borrow_mut();
        let fingerprint = s.fingerprints.get(path).filter(|f| !f.starts_with(thumbnails::PENDING)).cloned();
        let painter = s.painter.clone();
        let (lock, wake) = &*painter;
        let mut paint = lock.lock().expect("painter");
        if !paint.redraws.iter().any(|(p, _)| p == path) {
            paint.redraws.push_back((path.to_string(), fingerprint));
            s.drawing += 1;
        }
        wake.notify_one();
    }

    /// The open photo's thumbnail `REFRESH_DELAY` after its sidecar is written and no hand holds.
    fn schedule_redraw(self: &Rc<Self>) {
        let me = Rc::downgrade(self);
        self.refresher.start(TimerMode::SingleShot, thumbnails::REFRESH_DELAY, move || {
            let Some(me) = me.upgrade() else { return };
            let path = {
                let s = me.state.borrow();
                let held = s.hands.load(Ordering::Relaxed) > 0 || s.last_change.is_some();
                s.open.clone().filter(|_| !held)
            };
            if let Some(path) = path {
                me.redraw(&path);
            }
        });
    }

    /// The pass's count once it knows which photos lack a file.
    fn pass_counted(&self, generation: u64, held: usize, absent: usize) {
        let mut s = self.state.borrow_mut();
        if s.generation.load(Ordering::Relaxed) == generation {
            s.painted = (held, held + absent);
            s.creating = (0, absent);
        }
    }

    /// A drawn thumbnail lands in memory and on every tile holding its photo; a failure says why.
    fn thumb_landed(&self, pass: Option<u64>, fingerprint: String, drawn: Result<(Vec<u8>, usize, usize), String>, name: String) {
        let failure = {
            let mut s = self.state.borrow_mut();
            match pass {
                Some(g) if g == s.generation.load(Ordering::Relaxed) => {
                    s.painted.0 += 1;
                    s.creating.0 += 1;
                }
                Some(_) => {}
                None => s.drawing = s.drawing.saturating_sub(1),
            }
            match drawn {
                Ok((rgba, w, h)) => {
                    s.failed.remove(&fingerprint);
                    s.asked.remove(&fingerprint);
                    s.thumbs.insert(fingerprint, image(&rgba, w, h));
                    None
                }
                Err(e) => {
                    s.failed.insert(fingerprint);
                    Some(format!("{name}: {e}"))
                }
            }
        };
        if let Some(message) = failure {
            self.say(&message);
        }
        self.publish_tiles();
    }

    /// A file read from the folder; one absent stays asked, so only a render fills its tile.
    fn disk_landed(&self, fingerprint: String, read: Option<(Vec<u8>, usize, usize)>) {
        let Some((rgba, w, h)) = read else { return };
        {
            let mut s = self.state.borrow_mut();
            s.asked.remove(&fingerprint);
            s.thumbs.insert(fingerprint, image(&rgba, w, h));
        }
        self.publish_tiles();
    }

    /// `offerNeutralDecodeNotice`: once a library holds a RAW, until it is dismissed once.
    fn offer_neutral_decode_notice(&self, paths: &[String]) {
        let Some(w) = self.window() else { return };
        let seen = Preferences::load(&self.state.borrow().support).bool(preferences::NEUTRAL_DECODE_NOTICE_SHOWN).unwrap_or(false);
        if seen || crate::bench::requested().is_some() || !paths.iter().any(|p| oni_decode::is_raw(Path::new(p))) {
            return;
        }
        w.global::<crate::Notice>().set_shown(true);
    }

    /// Marked seen once dismissed, never on asking: a notice under another sheet would be spent unseen.
    fn neutral_decode_notice_dismissed(&self) {
        if let Some(w) = self.window() {
            w.global::<crate::Notice>().set_shown(false);
        }
        let support = self.state.borrow().support.clone();
        if let Err(e) = Preferences::load(&support).set(preferences::NEUTRAL_DECODE_NOTICE_SHOWN, true.into()) {
            self.say(&format!("The notice cannot be marked seen: {e}"));
        }
    }

    fn publish_tree(&self) {
        let Some(w) = self.window() else { return };
        let mut s = self.state.borrow_mut();
        let Some(archive) = &s.archive else { return };
        let rows = archive.rows();
        let paths: Vec<Option<String>> = rows
            .iter()
            .map(|r| match &r.id {
                oni_library::archive_tree::NodeId::Photo(p) => Some(p.clone()),
                _ => None,
            })
            .collect();
        let rows: Vec<TreeRow> = rows
            .into_iter()
            .zip(&paths)
            .map(|(r, path)| TreeRow {
                depth: r.depth as i32,
                title: r.title.into(),
                caption: r.caption.into(),
                kind: r.kind,
                expandable: r.expandable,
                expanded: r.expanded,
                selected: path.as_deref().is_some_and(|p| s.tree_marks(p)),
            })
            .collect();
        s.tree_paths = paths;
        drop(s);
        w.global::<Library>().set_rows(ModelRc::new(VecModel::from(rows)));
    }

    /// The tree's rows relit after a pick, in place: the list keeps its rows and its scroll.
    fn mark_tree(&self) {
        let Some(w) = self.window() else { return };
        let model = w.global::<Library>().get_rows();
        let s = self.state.borrow();
        if model.row_count() != s.tree_paths.len() {
            drop(s);
            return self.publish_tree();
        }
        for (i, path) in s.tree_paths.iter().enumerate() {
            let marked = path.as_deref().is_some_and(|p| s.tree_marks(p));
            if let Some(mut row) = model.row_data(i).filter(|r| r.selected != marked) {
                row.selected = marked;
                model.set_row_data(i, row);
            }
        }
    }

    fn row_clicked(self: &Rc<Self>, i: usize) {
        if self.state.borrow().org.trash_open {
            self.leave_trash();
            self.lay_grid();
        }
        let path = {
            let mut s = self.state.borrow_mut();
            let Some(archive) = s.archive.as_mut() else { return };
            let Some(row) = archive.rows().get(i).cloned() else { return };
            archive.toggle(&row.id)
        };
        match path {
            Some(path) => {
                self.state.borrow_mut().designate(&path);
                self.open_photo(&path);
                self.publish_tiles();
                self.publish_organising();
            }
            None => self.publish_tree(),
        }
    }

    /// Opens a photo in the store; the canvas renders once its negative is decoded.
    fn open_photo(self: &Rc<Self>, path: &str) {
        let failure = {
            let mut s = self.state.borrow_mut();
            let flush = s.store.flush();
            let opened = s.store.open(path).err();
            s.open = Some(path.to_string());
            s.opened_at = Instant::now();
            // Another photo's distributions never stand under this one's handles.
            s.inputs = None;
            s.shelf.hovered = None;
            s.reading = Some(render::Reading::of(s.store.settings()));
            s.inputs_drawn = None;
            s.rendered = 0;
            s.requested = 0;
            // Kept, not fitted: walking a roll at 100 % to compare grain needs it.
            s.viewport.recentre();
            s.requested_view = None;
            if s.full.as_ref().is_some_and(|(p, _)| p != path) {
                s.full = None;
            }
            for slide in &mut s.slides {
                slide.published = None;
            }
            flush.or(opened)
        };
        if let Some(message) = failure {
            self.say(&message);
        }
        if let Some(w) = self.window() {
            w.global::<Canvas>().set_busy(true);
            w.global::<Canvas>().set_status(format!("{} · decoding", stem(path)).into());
        }
        self.publish(true);
        self.publish_tree();
        self.schedule_redraw();
        self.decode_open(path);
    }

    /// The open frame's preview: the one kept, else a decode off this thread by the reading its
    /// settings ask for.
    fn decode_open(self: &Rc<Self>, path: &str) {
        let (previews, converted) = {
            let s = self.state.borrow();
            (s.previews.clone(), s.reading.clone().unwrap_or_else(|| render::Reading::of(s.store.settings())))
        };
        let known = previews.lock().expect("previews").get(path);
        if let Some(negative) = known {
            self.state.borrow_mut().negative = Some((path.to_string(), negative));
            return self.request_render();
        }
        self.state.borrow_mut().negative = None;
        let path = path.to_string();
        let engine = self.engine.clone();
        std::thread::spawn(move || {
            let prepared = prepared(&engine, &path, converted);
            with_shell(move |c| match prepared {
                Ok(negative) => {
                    c.state.borrow().previews.lock().expect("previews").insert(path.clone(), negative.clone());
                    if c.state.borrow().open_path() == Some(path.as_str()) {
                        c.state.borrow_mut().negative = Some((path, negative));
                        c.request_render();
                        c.publish_roll_tab();
                    }
                }
                Err(e) => {
                    if c.state.borrow().open_path() == Some(path.as_str()) {
                        c.deliver(Err(format!("{}: {e}", stem(&path))));
                    }
                }
            });
        });
    }

    /// `redecode()`: the open frame read again once its sidecar names another reading.
    fn redecode(self: &Rc<Self>) {
        let Some(path) = self.state.borrow().open.clone() else { return };
        {
            let mut s = self.state.borrow_mut();
            s.reading = Some(render::Reading::of(s.store.settings()));
            s.previews.lock().expect("previews").remove(&path);
            s.negative = None;
            s.full = None;
        }
        self.decode_open(&path);
    }

    // MARK: - The gallery

    fn relayout(&self) {
        {
            let mut s = self.state.borrow_mut();
            let groups: Vec<(String, String, usize)> =
                s.org.bands.iter().map(|b| (b.title.clone(), b.caption.clone(), b.entries.len())).collect();
            s.layout = Layout::new(&groups, s.columns, s.grid.0, Metrics::GALLERY);
            s.scroll = s.layout.clamped(s.scroll, s.grid.1);
        }
        self.publish_tiles();
    }

    /// The cells in view and one row either side, the only tiles Slint is given.
    fn publish_tiles(&self) {
        // Every change of the picks lays the tiles: the sidebar's rows follow them here.
        self.mark_tree();
        let Some(w) = self.window() else { return };
        let lib = w.global::<Library>();
        let mut s = self.state.borrow_mut();
        let range = s.layout.cells(s.scroll, s.grid.1);
        let flags = Ctl::tile_flags(&s);
        // A cell reads the folder, never renders: a photo with no file waits for the pass or an edit.
        let mut asking = Vec::new();
        for i in range.clone() {
            let Some(fingerprint) = s.order.get(i).and_then(|p| s.fingerprints.get(p)) else { continue };
            if !fingerprint.starts_with(thumbnails::PENDING) && !s.thumbs.contains(fingerprint) && !s.asked.contains(fingerprint) {
                asking.push(fingerprint.clone());
            }
        }
        for fingerprint in asking {
            s.asked.insert(fingerprint.clone());
            let _ = s.disk.send(fingerprint);
        }
        let tiles: Vec<Tile> = range
            .clone()
            .filter_map(|i| {
                let frame = s.layout.frame(i)?;
                let path = s.order.get(i)?;
                let picture = s.fingerprints.get(path).and_then(|f| s.thumbs.get(f)).unwrap_or_default();
                Some(Tile {
                    index: i as i32,
                    x: frame.x,
                    y: frame.y,
                    width: frame.width,
                    height: s.layout.tile.1,
                    picture,
                    caption: stem(path).into(),
                    selected: s.shown_picks().selection.contains(path),
                    medal: flags.get(path).map_or(0, |f| f.0),
                    missing: flags.get(path).is_some_and(|f| f.1),
                    rgb: flags.get(path).is_some_and(|f| f.2),
                })
            })
            .collect();
        let headers = s.layout.headers(s.scroll, s.grid.1);
        let headings: Vec<Heading> = s.layout.bands[headers]
            .iter()
            .map(|b| Heading { y: b.top, title: b.title.clone().into(), caption: b.caption.clone().into() })
            .collect();
        s.visible = range;
        let (height, scroll) = (s.layout.height, s.scroll);
        drop(s);
        lib.set_tiles(ModelRc::new(VecModel::from(tiles)));
        lib.set_headings(ModelRc::new(VecModel::from(headings)));
        lib.set_content_height(height);
        lib.set_scroll(scroll);
    }

    fn tile_clicked(self: &Rc<Self>, i: usize, toggle: bool, extend: bool) {
        self.click(i, toggle, extend);
    }

    fn tile_opened(self: &Rc<Self>, i: usize) {
        let path = self.state.borrow().order.get(i).cloned();
        if let Some(path) = path {
            self.open_in_editor(&path);
        }
    }

    /// `load(_:)` then `mode = .editor`.
    fn open_in_editor(self: &Rc<Self>, path: &str) {
        self.state.borrow_mut().designate(path);
        self.open_photo(path);
        if let Some(w) = self.window() {
            w.global::<App>().set_gallery(false);
        }
        self.publish_bar();
        self.publish_tiles();
    }

    /// `applyMedal`: the selection's sidecars and index rows, in the gallery and the editor alike.
    fn label(self: &Rc<Self>, medal: i32) {
        let targets = self.state.borrow().selected();
        self.label_paths(&targets, library::medal_of(medal));
    }

    fn label_paths(self: &Rc<Self>, targets: &[String], label: Medal) {
        let mut written = Vec::new();
        for path in targets {
            match oni_settings::sidecar::set_label(label, Path::new(path), true) {
                Ok(()) => written.push(path.clone()),
                Err(e) => self.say(&e.to_string()),
            }
        }
        self.reread(&written);
    }

    /// Rows whose sidecar a gesture wrote, read again into the index, then the gallery laid from
    /// the index, so a photo marked or restored leaves or joins it.
    fn reread(self: &Rc<Self>, paths: &[String]) {
        if paths.is_empty() {
            return;
        }
        {
            let mut s = self.state.borrow_mut();
            let Some(folder) = s.opened.clone() else { return };
            let Some(index) = &s.index else { return };
            library::refresh(index, paths);
            let mut rows = library::shown_under(index, &folder);
            let held: HashSet<String> = rows.iter().map(|e| e.path.clone()).collect();
            let waiting: Vec<oni_library::Entry> =
                s.rows.iter().filter(|r| r.fingerprint.starts_with(thumbnails::PENDING) && !held.contains(&r.path)).cloned().collect();
            rows.extend(waiting);
            s.fingerprints = rows.iter().map(|e| (e.path.clone(), e.fingerprint.clone())).collect();
            s.rows = rows;
        }
        self.lay_archive();
    }

    // MARK: - The phone

    /// The link's Host on its own thread, once; its events come back to this thread one by one.
    fn start_phone(self: &Rc<Self>) {
        if self.state.borrow().phone.is_some() {
            return;
        }
        let support = self.state.borrow().support.clone();
        match Phone::start(&support, |event| with_shell(move |c| c.phone_said(event))) {
            Ok(p) => self.state.borrow_mut().phone = Some(p),
            Err(e) => self.pairing_status(&format!("The phone link cannot start: {e}")),
        }
    }

    /// The sheet with a fresh one-time code, once the Host listens.
    fn pair_phone(self: &Rc<Self>) {
        if let Some(w) = self.window() {
            let p = w.global::<Pairing>();
            p.set_code(Image::default());
            p.set_uri(SharedString::new());
            p.set_shown(true);
        }
        self.pairing_status("Starting the link…");
        self.start_phone();
        self.show_code();
    }

    fn show_code(&self) {
        let Some(w) = self.window() else { return };
        if !w.global::<Pairing>().get_shown() {
            return;
        }
        let code = self.state.borrow().phone.as_ref().and_then(Phone::open_pairing);
        let Some(code) = code else { return };
        if code.addresses.is_empty() {
            return self.pairing_status("No local network: join the phone's Wi-Fi network, then pair again.");
        }
        let uri = code.to_uri();
        match phone::qr(&uri) {
            Ok((rgba, side)) => {
                let p = w.global::<Pairing>();
                p.set_code(image(&rgba, side, side));
                p.set_uri(uri.into());
                let at: Vec<String> = code.addresses.iter().map(ToString::to_string).collect();
                self.pairing_status(&format!("Scan it from the phone app on the same network. One phone, two minutes · {}", at.join(", ")));
            }
            Err(e) => self.pairing_status(&format!("The code cannot be drawn: {e}")),
        }
    }

    fn close_pairing(&self) {
        if let Some(p) = self.state.borrow().phone.as_ref() {
            p.close_pairing();
        }
        if let Some(w) = self.window() {
            w.global::<Pairing>().set_shown(false);
        }
    }

    fn pairing_status(&self, text: &str) {
        if let Some(w) = self.window() {
            w.global::<Pairing>().set_status(text.into());
        }
    }

    /// The store's screen to the phone, the open frame's place with it; unchanged, nothing is sent.
    fn refresh_phone(&self) {
        let s = &mut *self.state.borrow_mut();
        let place = s.place();
        let (Some(p), Some(store)) = (s.phone.as_mut(), s.store.editor()) else { return };
        p.link.frame = place;
        p.link.refresh(store);
    }

    fn phone_said(self: &Rc<Self>, event: phone::Event) {
        match event {
            phone::Event::Listening(at) => {
                timing::line(&format!("phone link listening on {at}"));
                if let Some(p) = self.state.borrow_mut().phone.as_mut() {
                    p.listening(at);
                }
                self.refresh_phone();
                self.show_code();
            }
            phone::Event::Failed(e) => {
                timing::line(&format!("phone link failed: {e}"));
                self.state.borrow_mut().phone = None;
                self.pairing_status(&format!("The phone link cannot listen: {e}"));
            }
            phone::Event::Host(event) => self.phone_event(event),
        }
    }

    /// A press lands on the store as one command and one undo step, then the window follows.
    fn phone_event(self: &Rc<Self>, event: HostEvent) {
        let (out, changed) = {
            let s = &mut *self.state.borrow_mut();
            let place = s.place();
            let revision = s.store.revision();
            let (Some(p), Some(store)) = (s.phone.as_mut(), s.store.editor()) else { return };
            p.link.frame = place;
            let out = p.link.handle(store, event);
            (out, store.revision() != revision)
        };
        if changed {
            self.state.borrow_mut().last_change = Some(Instant::now());
            self.publish(false);
        }
        match out {
            Some(Outward::Paired { phone, name }) => {
                let support = self.state.borrow().support.clone();
                if let Err(e) = phone::trust(&support, phone) {
                    self.say(&format!("{name} is paired for this session only: {e}"));
                }
                self.pairing_status(&format!("Paired with {name}."));
            }
            Some(Outward::Connected { name, .. }) => {
                timing::line(&format!("phone connected: {name}"));
                self.say(&format!("{name} connected"));
                self.pairing_status(&format!("{name} is connected."));
            }
            Some(Outward::Disconnected { .. }) => {
                timing::line("phone disconnected");
                self.say("The phone disconnected");
            }
            Some(Outward::Walk(by)) => self.walk(by),
            None => {}
        }
    }

    /// `walk(by:)`: the neighbour in the open frame's group, opened and selected; the ends stop it.
    fn walk(self: &Rc<Self>, by: i8) {
        let target = {
            let s = self.state.borrow();
            let Some((at, count)) = s.place() else { return };
            let to = (i32::from(at) - 1 + i32::from(by)).clamp(0, i32::from(count) - 1) as usize;
            let open = s.open_path().unwrap_or_default();
            let group = s.archive.as_ref().and_then(|a| a.groups.iter().find(|g| g.entries.iter().any(|e| e.path == open)));
            group.and_then(|g| g.entries.get(to)).map(|e| e.path.clone()).filter(|p| p != open)
        };
        let Some(path) = target else { return };
        self.state.borrow_mut().designate(&path);
        self.open_photo(&path);
        self.publish_tiles();
    }

    // MARK: - Gestures

    /// Bar keys, menu items and shortcuts meet here; what this build does not carry says so.
    pub fn act(self: &Rc<Self>, action: &str) {
        let Some(w) = self.window() else { return };
        let app = w.global::<App>();
        match action {
            "undo" | "redo" if app.get_gallery() => self.gallery_step(action == "undo"),
            "undo" => self.write(Command::Undo),
            "redo" => self.write(Command::Redo),
            "reset" => self.reset_all(),
            "toggle-mode" if self.export.borrow().shown => {}
            "export" => self.export_act(action),
            "toggle-mode" => self.toggle_mode(),
            "hide-panel" => app.set_panel_hidden(!app.get_panel_hidden()),
            "hide-library" => app.set_library_hidden(!app.get_library_hidden()),
            "smaller" | "larger" if app.get_gallery() => {
                let by = if action == "larger" { 1 } else { -1 };
                let columns = gallery::column_step(self.state.borrow().columns, by);
                self.state.borrow_mut().columns = columns;
                self.relayout();
            }
            "smaller" | "larger" => self.step_zoom(if action == "larger" { 1 } else { -1 }),
            "compare" if !app.get_gallery() && self.state.borrow().open.is_some() => self.toggle_compare(),
            "screen-gamut" => {
                self.toggle_screen_gamut();
                self.gamut_kept();
            }
            "bigger-brush" | "smaller-brush" if self.painting() => {
                // `EditorScene.biggerBrush`: four pixels a key, inside the brush's range.
                let by = if action == "bigger-brush" { 4.0 } else { -4.0 };
                let (low, high) = oni_settings::frame::Correction::RADIUS_RANGE;
                {
                    let mut s = self.state.borrow_mut();
                    s.brush = (f64::from(s.brush) + by).clamp(low, high) as f32;
                }
                self.publish_dedust();
            }
            "bigger-brush" | "smaller-brush" => {}
            "correction" if self.state.borrow().open.is_some() => self.toggle_correcting(),
            "framing" if self.state.borrow().open.is_some() => self.toggle_framing(),
            "mirror" => self.framing_act("mirror"),
            "toggle-sound" => {
                let mut s = self.state.borrow_mut();
                s.muted = !s.muted;
            }
            "cycle-filter" => self.cycle_filter(),
            "show-selected" => self.show_selected(),
            "copy" => {
                if let Some(store) = self.state.borrow_mut().store.editor() {
                    store.copy_settings();
                }
            }
            "paste" => self.paste(),
            "paste-to-roll" => self.paste_to_roll(),
            "copy-framing" => {
                if let Some(store) = self.state.borrow_mut().store.editor() {
                    store.copy_framing();
                }
                self.publish_framing();
            }
            "paste-framing" | "paste-framing-to-roll" => self.paste_framing(action == "paste-framing-to-roll"),
            "remove" => self.move_to_trash(),
            "restore" => {
                let picked: Vec<String> = {
                    let s = self.state.borrow();
                    s.order.iter().filter(|p| s.trash_picks.selection.contains(*p)).cloned().collect()
                };
                self.restore(&picked);
            }
            "new-roll" if app.get_gallery() => self.new_roll(),
            "reset-to-default" if app.get_gallery() => self.ask_reset_to_default(),
            "ungroup" => self.ungroup(),
            "confirm-yes" | "confirm-no" => self.answered(action == "confirm-yes"),
            "open" => self.pick(),
            "recheck" | "read-folders" => self.read_again(),
            "auto-balance" => self.auto_balance(),
            "save-preset" => self.save_preset(),
            "edit-presets" => self.edit_presets(None),
            "import-presets" => self.import_presets(),
            "paste-preset-code" => self.paste_preset_code(),
            "show-presets" => self.show_presets_folder(),
            "pair-phone" => self.pair_phone(),
            "close-pairing" => self.close_pairing(),
            a if a.starts_with("medal-") => {
                if let Ok(m) = a[6..].parse::<i32>() {
                    self.label(m);
                }
            }
            a if self.lab_act(a) => {}
            other => self.say(&format!("“{other}” is not in this build yet")),
        }
        self.publish_bar();
    }

    /// `QuitGuard.applicationShouldTerminate`: the sidecar written before the window goes; a write
    /// that fails asks first, Cancel keeping the app open to free space or reconnect the volume.
    fn close_requested(self: &Rc<Self>) -> slint::CloseRequestResponse {
        if self.state.borrow().quitting {
            return slint::CloseRequestResponse::HideWindow;
        }
        let failure = self.state.borrow_mut().store.flush();
        let Some(message) = failure else { return slint::CloseRequestResponse::HideWindow };
        self.ask(
            "Your latest settings could not be saved",
            &format!("{message}\n\nQuitting now discards the changes made since the last successful save. Cancel to keep the app open and fix the problem \u{2014} for instance by freeing space or reconnecting the volume the originals live on."),
            "Quit Anyway",
            Pending::QuitAnyway,
        );
        slint::CloseRequestResponse::KeepWindowShown
    }

    /// The keys no menu item carries: Escape and Return on a sheet, the Lab's and the walk keys, and
    /// ⌘=, which types apart from the menu's ⌘+; every other shortcut is the menu bar's.
    fn key(self: &Rc<Self>, text: &str, command: bool, shift: bool, bare: bool, repeat: bool) -> bool {
        let Some(w) = self.window() else { return false };
        if w.global::<Confirm>().get_shown() {
            // Quitting is the one sheet whose default is Cancel: Return never discards unsaved work.
            let safe = matches!(self.state.borrow().pending, Some(Pending::QuitAnyway));
            match text {
                "\u{1b}" => self.answered(false),
                "\r" | "\n" => self.answered(!safe),
                _ => {}
            }
            return true;
        }
        if w.global::<crate::Choice>().get_shown() {
            if text == "\u{1b}" {
                w.global::<crate::Choice>().invoke_chosen(-1);
            }
            return true;
        }
        if w.global::<crate::Advice>().get_shown() {
            if matches!(text, "\u{1b}" | "\r" | "\n") {
                self.advice_closed();
            }
            return true;
        }
        if self.preset_form_key(text) {
            return true;
        }
        if text == "\u{1b}" && w.global::<Pairing>().get_shown() {
            self.close_pairing();
            return true;
        }
        if text == "\u{1b}" && self.export_escape() {
            return true;
        }
        if self.lab_key(text, bare, repeat) {
            return true;
        }
        if w.global::<App>().get_gallery() && self.grid_key(text, command, shift, bare, repeat) {
            return true;
        }
        // `keyboardShortcut("+")` answers ⌘= too, the key that carries + unshifted.
        if command && !shift && text == "=" && w.global::<Menus>().get_live().larger {
            self.act("larger");
            return true;
        }
        false
    }

    /// `propagationTargets`: the selection, else the open photo; the ONI Trash's never are.
    fn targets(&self) -> Vec<String> {
        let s = self.state.borrow();
        if s.picks.selection.is_empty() {
            return s.open.clone().into_iter().collect();
        }
        s.selected()
    }

    /// A gesture on several photos through the core, then what it wrote read back: the rows, the
    /// open frame's sliders, the others' thumbnails.
    fn gesture(self: &Rc<Self>, touched: &[String], run: impl FnOnce(&mut oni_core::EditorStore) -> usize) {
        let failure = {
            let mut s = self.state.borrow_mut();
            let Some(store) = s.store.editor() else { return };
            run(store);
            store.failure.take()
        };
        if let Some(message) = failure {
            self.say(&message);
        }
        self.landed(touched);
    }

    fn landed(self: &Rc<Self>, touched: &[String]) {
        self.closed_if_trashed();
        self.publish(false);
        self.reread(touched);
        let open = self.state.borrow().open.clone();
        for path in touched.iter().filter(|p| Some(*p) != open.as_ref()) {
            self.redraw(path);
        }
    }

    /// `took(_:at:)`: a trashed open frame leaves the editor, which falls back on the gallery.
    fn closed_if_trashed(&self) {
        let closed = {
            let mut s = self.state.borrow_mut();
            let held = s.store.editor().map(|e| e.path().map(str::to_string));
            match held {
                Some(None) if s.open.is_some() => {
                    s.open = None;
                    s.negative = None;
                    true
                }
                _ => false,
            }
        };
        if closed && let Some(w) = self.window() {
            w.global::<App>().set_gallery(true);
            w.global::<Canvas>().set_frame(Image::default());
        }
    }

    /// `undo()`/`redo()` in the gallery: the gallery's own stack, whichever photos it touched.
    fn gallery_step(self: &Rc<Self>, undo: bool) {
        let (touched, moves): (Vec<String>, Vec<Move>) = {
            let mut s = self.state.borrow_mut();
            let Some(store) = s.store.editor() else { return };
            let history = store.gallery_history();
            let paths = if undo { history.undo_paths() } else { history.redo_paths() };
            let moves = if undo { history.undo_moves() } else { history.redo_moves() };
            (paths.into_iter().collect(), moves)
        };
        if !moves.is_empty() {
            // The open frame's pending write lands before its file moves.
            let failure = self.state.borrow_mut().store.flush();
            if let Some(message) = failure {
                self.say(&message);
            }
        }
        self.gesture(&touched, |store| {
            if undo { store.undo(Surface::Gallery) } else { store.redo(Surface::Gallery) };
            0
        });
        if !moves.is_empty() {
            let made: Vec<Move> = moves.into_iter().filter(|m| Path::new(&m.to).exists() && !Path::new(&m.from).exists()).collect();
            self.followed(&made);
        }
    }

    /// `rollFolder(from:)`: a dated folder beside the first target, the photos, their passes and
    /// sidecars moved in, one gallery step that ⌘Z walks back.
    fn new_roll(self: &Rc<Self>) {
        let targets: HashSet<String> = self.targets().into_iter().collect();
        self.new_roll_from(&targets);
    }

    /// `rollFolder(from:)`: the photos moved into a dated roll folder beside the first, one step.
    fn new_roll_from(self: &Rc<Self>, targets: &HashSet<String>) {
        let (ordered, earliest) = {
            let s = self.state.borrow();
            let ordered: Vec<String> = s.order.iter().filter(|p| targets.contains(*p)).cloned().collect();
            let dates = s.rows.iter().filter(|e| targets.contains(&e.path)).filter_map(|e| e.captured_at);
            (ordered, dates.reduce(|a, b| if b < a { b } else { a }))
        };
        let Some(first) = ordered.first() else { return };
        let parent = oni_library::paths::parent(first);
        let name = oni_library::roll_folder::name_for(earliest.unwrap_or_else(oni_settings::Date::now), &parent);
        let film = oni_settings::Film::new(&name);
        let folder = match oni_library::roll_folder::make(&name, &parent, &film) {
            Ok(folder) => folder,
            Err(e) => return self.say(&format!("The roll's folder could not be made: {e}")),
        };
        let files = oni_library::rgb_passes::expanded(&ordered);
        let failure = self.state.borrow_mut().store.flush();
        if let Some(message) = failure {
            self.say(&message);
        }
        let made = oni_library::roll_folder::move_items(&oni_library::roll_folder::moves(&files, &folder));
        if made.len() < files.len() {
            self.say(&format!("{} of the files could not be moved into the roll.", files.len() - made.len()));
        }
        if let Some(store) = self.state.borrow_mut().store.editor() {
            store.record_disk(oni_core::store::Disk { folder: Some(folder), film: Some(film), moves: made.clone() }, "New Roll");
        }
        self.followed(&made);
    }

    /// `followed(_:)`: the index rows, the selection and the open frame go where the disk moved
    /// them, then the folder is read again for its new tree.
    fn followed(self: &Rc<Self>, moves: &[Move]) {
        let moved = |path: &str| {
            moves.iter().find_map(|m| {
                if path == m.from {
                    Some(m.to.clone())
                } else {
                    oni_library::paths::holds(&m.from, path).then(|| format!("{}{}", m.to, &path[m.from.len()..]))
                }
            })
        };
        let reopen = {
            let mut s = self.state.borrow_mut();
            if let Some(index) = &s.index {
                for row in index.all() {
                    if let Some(to) = moved(&row.path) {
                        let _ = index.remove(&row.path);
                        let _ = index.upsert(&oni_library::Entry { path: to, ..row });
                    }
                }
            }
            for row in &mut s.rows {
                if let Some(to) = moved(&row.path) {
                    row.path = to;
                }
            }
            let fingerprints = s.rows.iter().map(|e| (e.path.clone(), e.fingerprint.clone())).collect();
            s.fingerprints = fingerprints;
            for path in &mut s.order {
                if let Some(to) = moved(path) {
                    *path = to;
                }
            }
            s.open.as_deref().and_then(|p| moved(p))
        };
        self.lay_archive();
        if let Some(to) = reopen {
            self.open_photo(&to);
        }
        self.read_again();
    }

    /// `resetAll()`: every target back to its imported state, one gallery step.
    fn reset_all(self: &Rc<Self>) {
        let targets = self.targets();
        self.gesture(&targets, |store| store.reset_frames(&targets));
    }

    /// A mode change drops the split, as Swift's does.
    fn drop_split(self: &Rc<Self>) {
        if self.state.borrow().split.is_some() {
            self.toggle_compare();
        }
    }

    /// `toggleMode()`: the editor on the gallery's ringed photo, or back to the gallery.
    fn toggle_mode(self: &Rc<Self>) {
        let Some(w) = self.window() else { return };
        // Leaving the editor takes the deck away with no exit event: the preview goes with it.
        self.state.borrow_mut().shelf.hovered = None;
        if !w.global::<App>().get_gallery() {
            self.drop_split();
            w.global::<App>().set_gallery(true);
            self.publish_tiles();
            return;
        }
        let ringed = {
            let s = self.state.borrow();
            s.picks.anchor.clone().filter(|_| !s.trash_shown())
        };
        let Some(path) = ringed else { return };
        self.drop_split();
        if self.state.borrow().open.as_deref() == Some(path.as_str()) {
            w.global::<App>().set_gallery(false);
        } else {
            self.open_in_editor(&path);
        }
    }

    /// `toggleFraming()`: into the crop, or out to the last panel the hand was in, never the crop.
    fn toggle_framing(self: &Rc<Self>) {
        let Some(w) = self.window() else { return };
        let panel = w.global::<PanelState>();
        let tab = panel.get_tab();
        let next = if tab == CROP_TAB {
            let last = self.state.borrow().last_panel;
            if last == CROP_TAB || last == 4 { 2 } else { last }
        } else {
            if tab != 4 {
                self.state.borrow_mut().last_panel = tab;
            }
            CROP_TAB
        };
        panel.set_tab(next);
        w.global::<Bar>().set_dedust(false);
        self.tab_opened(next);
    }

    /// nil, bronze, silver, gold, then nil again: exactly one medal, never a tier above.
    fn cycle_filter(self: &Rc<Self>) {
        {
            let mut s = self.state.borrow_mut();
            s.filter = match s.filter {
                None => Some(Medal::Bronze),
                Some(Medal::Bronze) => Some(Medal::Silver),
                Some(Medal::Silver) => Some(Medal::Gold),
                _ => None,
            };
        }
        self.lay_archive();
    }

    /// `centreSelected()`: the open photo, else the first selected, scrolled into view.
    fn show_selected(self: &Rc<Self>) {
        {
            let mut s = self.state.borrow_mut();
            let at = s.open.as_ref().and_then(|p| s.order.iter().position(|o| o == p)).or_else(|| s.order.iter().position(|o| s.picks.selection.contains(o)));
            let Some(at) = at else { return };
            if let Some(top) = s.layout.reveal(at, s.scroll, s.grid.1) {
                s.scroll = top;
            }
        }
        self.publish_tiles();
    }

    /// `pasteSettings(to:)`: one roll's look laid on another asks first.
    fn paste(self: &Rc<Self>) {
        let targets = self.targets();
        let check = {
            let mut s = self.state.borrow_mut();
            let Some(store) = s.store.editor() else { return };
            store.paste_check(&targets)
        };
        match check {
            PasteCheck::Nothing => {}
            PasteCheck::Ready => self.lay_paste(targets),
            PasteCheck::AnotherRoll => self.confirm("Paste Settings", Pending::Paste(targets)),
        }
    }

    fn lay_paste(self: &Rc<Self>, targets: Vec<String>) {
        self.gesture(&targets, |store| store.paste_settings(&targets));
    }

    /// `pasteToRoll()`: a frame measured with no roll takes the paste alone; another roll's look asks first.
    fn paste_to_roll(self: &Rc<Self>) {
        let check = {
            let mut s = self.state.borrow_mut();
            let Some(open) = s.open.clone() else { return };
            let roll = s.store.settings().roll_reference.is_some();
            let Some(store) = s.store.editor() else { return };
            if !store.has_copied_settings() {
                return;
            }
            if roll { store.paste_check(&[open]) } else { PasteCheck::Ready }
        };
        match check {
            PasteCheck::AnotherRoll => self.confirm("Paste to Roll", Pending::PasteToRoll),
            _ => self.lay_paste_to_roll(),
        }
    }

    fn lay_paste_to_roll(self: &Rc<Self>) {
        let touched: Vec<String> = {
            let mut s = self.state.borrow_mut();
            let open = s.open.clone();
            let id = s.store.settings().roll_reference.as_ref().map(|r| r.id);
            let Some(store) = s.store.editor() else { return };
            store.paste_to_roll();
            let mut touched: Vec<String> = id.and_then(|id| store.measured_group(id)).map(|g| g.iter().cloned().collect()).unwrap_or_default();
            touched.extend(open);
            touched
        };
        self.gesture(&touched, |_| 0);
    }

    /// `pasteFramingToRoll()`: the open frame first, then every frame measured with its roll.
    fn paste_framing_to_roll(self: &Rc<Self>) {
        let targets: Vec<String> = {
            let mut s = self.state.borrow_mut();
            let Some(open) = s.open.clone() else { return };
            let id = s.store.settings().roll_reference.as_ref().map(|r| r.id);
            let Some(store) = s.store.editor() else { return };
            let members = id.map(|id| store.measured_with(id)).unwrap_or_default();
            std::iter::once(open.clone()).chain(members.into_iter().filter(|m| *m != open)).collect()
        };
        let size = self.image_size();
        self.gesture(&targets, |store| store.paste_framing(&targets, size));
    }

    /// `moveToOniTrash()`: each target's sidecar marked, its files untouched, one gallery step.
    fn move_to_trash(self: &Rc<Self>) {
        let targets = self.targets();
        self.move_paths_to_trash(&targets);
    }

    fn move_paths_to_trash(self: &Rc<Self>, targets: &[String]) {
        let targets = targets.to_vec();
        if targets.is_empty() {
            return;
        }
        self.gesture(&targets, |store| store.move_to_trash(&targets));
    }

    /// The film type's tabs: `applyMode` on the selection, else the open photo.
    fn film_type(self: &Rc<Self>, index: i32) {
        let Some(mode) = usize::try_from(index).ok().and_then(|i| ConversionMode::ALL.get(i).copied()) else { return };
        let targets = self.targets();
        self.gesture(&targets, |store| store.apply_mode(mode, &targets));
    }

    /// `DSDialogRequest.confirm`: the sheet over the window until a key answers it.
    fn confirm(&self, key: &str, pending: Pending) {
        self.state.borrow_mut().pending = Some(pending);
        if let Some(w) = self.window() {
            let c = w.global::<Confirm>();
            c.set_title(ANOTHER_ROLL.0.into());
            c.set_message(ANOTHER_ROLL.1.into());
            c.set_key(key.to_uppercase().into());
            c.set_shown(true);
        }
    }

    fn answered(self: &Rc<Self>, go: bool) {
        if let Some(w) = self.window() {
            w.global::<Confirm>().set_shown(false);
        }
        let pending = self.state.borrow_mut().pending.take();
        match pending.filter(|_| go) {
            Some(Pending::Paste(targets)) => self.lay_paste(targets),
            Some(Pending::PasteToRoll) => self.lay_paste_to_roll(),
            Some(Pending::DeleteProject(id)) => self.delete_project(id),
            Some(Pending::RestoreAll) => self.restore_all(),
            Some(Pending::DeletePreset(id)) => self.delete_preset(id),
            Some(Pending::UpdatePreset(id)) => self.update_preset(id),
            Some(Pending::ResetToDefault(reach)) => self.reset_to_default(reach),
            Some(Pending::QuitAnyway) => {
                self.state.borrow_mut().quitting = true;
                let _ = slint::quit_event_loop();
            }
            None => {}
        }
        self.publish_bar();
    }

    /// `menuState`: every item's title and whether a gesture can reach it now, from the state the
    /// bar reads too, so the two surfaces cannot disagree.
    fn publish_menus(&self) {
        let Some(w) = self.window() else { return };
        let app = w.global::<App>();
        let (gallery, library_hidden, panel_hidden) = (app.get_gallery(), app.get_library_hidden(), app.get_panel_hidden());
        let dedust = w.global::<PanelState>().get_tab() == 4;
        let (painting, canvas) = (self.painting(), self.canvas_size());
        let (titles, live, film_type, targets) = {
            let mut s = self.state.borrow_mut();
            let open = s.open.is_some();
            let selected = s.picks.selection.len();
            let trash = s.trash_shown() && gallery;
            let restoring = s.trash_picks.selection.len();
            let count = if selected > 0 { selected } else { usize::from(open) };
            let anchor = s.picks.anchor.is_some();
            // `modalOpen`: the export surface, a preset sheet or a dialog.
            let modal = self.export.borrow().shown || s.shelf_form_open() || w.global::<Confirm>().get_shown() || w.global::<crate::Advice>().get_shown();
            let presets = s.shelf.held.len();
            let members = s.roll_members(gallery).len();
            let (opened, read_done, filter, muted, columns) = (s.opened.is_some(), s.read_done, s.filter, s.muted, s.columns);
            let (split, screen_gamut, decoded) = (s.split.is_some(), s.screen_gamut, s.negative.is_some());
            // `zoomRung(by:)`: a step of the ladder, until the canvas is measured or at either end.
            let rung = |by: i32| match (s.full_size, canvas) {
                (Some(full), Some(view)) if !gallery && decoded => Viewport::rung(s.viewport.effective_scale(full, view), s.viewport.is_fitted(), by).is_some(),
                _ => false,
            };
            let (zoom_out, zoom_in) = (rung(-1), rung(1));
            let settings = s.store.settings().clone();
            let editor_undo = (s.store.can_undo(), s.store.can_redo());
            let open_path = s.open.clone();
            let store = s.store.editor();
            let gallery_history = store.as_ref().map(|e| {
                let h = e.gallery_history();
                (h.can_undo(), h.can_redo(), h.undo_label().map(str::to_string), h.redo_label().map(str::to_string))
            });
            let (has_settings, scanner_face) =
                store.as_ref().map_or((false, false), |e| (e.has_copied_settings(), e.scanner_face));
            let reach = settings
                .roll_reference
                .as_ref()
                .and_then(|r| store.as_ref().and_then(|e| e.measured_group(r.id)))
                .map_or(0, |m| m.len() - usize::from(open_path.as_ref().is_some_and(|p| m.contains(p))));
            let (can_undo, can_redo, undo_title, redo_title) = match (gallery, gallery_history) {
                (true, Some((u, r, ul, rl))) => {
                    (u, r, ul.map_or("Undo".into(), |l| format!("Undo {l}")), rl.map_or("Redo".into(), |l| format!("Redo {l}")))
                }
                (true, None) => (false, false, "Undo".to_string(), "Redo".to_string()),
                (false, _) => (editor_undo.0, editor_undo.1, "Undo".to_string(), "Redo".to_string()),
            };
            let can_reset = count > 0 && (count != 1 || !open || settings != oni_core::parameters::reset_blank(&settings, scanner_face));
            let (exporting, writing) = {
                let c = self.export.borrow();
                (c.shown.then_some(c.targets.len()), c.exporting)
            };
            let plural = |n: usize, one: &str, many: String| if n > 1 { many } else { one.to_string() };
            let titles = MenuTitles {
                mode: if gallery { "Editor" } else { "Gallery" }.into(),
                export: crate::export::label::menu(exporting.unwrap_or(selected)).into(),
                remove: plural(count, "Move to ONI Trash", format!("Move {count} Photos to ONI Trash")).into(),
                restore: plural(restoring, "Restore", format!("Restore {restoring} Photos")).into(),
                paste: plural(count, "Paste Settings", format!("Paste Settings to {count} Photos")).into(),
                paste_to_roll: if reach > 0 { format!("Paste to Roll ({} Photos)", reach + 1) } else { "Paste to Roll".into() }.into(),
                undo: undo_title.into(),
                redo: redo_title.into(),
                smaller: if gallery { "Smaller Thumbnails" } else { "Zoom Out" }.into(),
                larger: if gallery { "Larger Thumbnails" } else { "Zoom In" }.into(),
                sound: if muted { "Unmute Key Clicks" } else { "Mute Key Clicks" }.into(),
                compare: if split { "Close Comparison" } else { "Compare Before/After" }.into(),
                library: if library_hidden { "Show the Library" } else { "Hide the Library" }.into(),
                panel: if panel_hidden { "Show the Panel" } else { "Hide the Panel" }.into(),
                correcting: if dedust { "Done Correcting" } else { "Remove Dust & Scratches" }.into(),
                mirror: if settings.geometry.mirrored { "Unmirror" } else { "Mirror Horizontally" }.into(),
                filter: filter.map_or("Filter: All".into(), |m| format!("Filter: {}", library::medal(m))).into(),
                roll: plural(count, "New Roll from Selection", format!("New Roll from {count} Photos")).into(),
                ungroup: plural(members, "Ungroup", format!("Ungroup {members} Photos")).into(),
                sheet: plural(selected, "New Contact Sheet", format!("New Contact Sheet from {selected} Photos")).into(),
                screen_gamut: if screen_gamut { "Hide What This Screen Cannot Show" } else { "Show What This Screen Cannot Show" }.into(),
            };
            // A gesture this build lacks stays dark, whatever Swift's rule would say.
            let live = MenuLive {
                open: true,
                toggle_mode: !gallery || anchor,
                // On the surface ⌘E confirms; elsewhere it opens on the selection, else the open photo.
                export: exporting.map_or(open || selected > 0, |n| n > 0 && !writing),
                recheck: read_done || !opened,
                read_folders: opened,
                show_selected: gallery && (open || selected > 0),
                remove: count > 0,
                restore: trash && restoring > 0,
                undo: can_undo,
                redo: can_redo,
                reset: can_reset,
                smaller: if gallery { gallery::column_step(columns, -1) != columns } else { zoom_out },
                larger: if gallery { gallery::column_step(columns, 1) != columns } else { zoom_in },
                compare: !gallery && decoded,
                hide_library: !gallery,
                hide_panel: !gallery,
                framing: open && !gallery,
                correction: open,
                bigger_brush: painting,
                smaller_brush: painting,
                mirror: open,
                screen_gamut: true,
                emulator: false,
                medal: selected > 0 && !modal,
                cycle_filter: true,
                new_roll: gallery && count > 0,
                ungroup: members > 0,
                contact_sheet: false,
                // Dark until Prepare the Roll carries its surface.
                measure_roll: false,
                reset_to_default: gallery && count > 0,
                copy_settings: open,
                paste_settings: has_settings && count > 0,
                paste_to_roll: has_settings && open,
                save_preset: open,
                edit_presets: !modal && presets > 0,
                import_presets: !modal,
                show_presets: !modal,
                paste_preset_code: !modal,
                apply_preset: count > 0,
            };
            let film_type = ConversionMode::ALL.iter().position(|m| *m == settings.mode).unwrap_or(0) as i32;
            (titles, live, film_type, count)
        };
        let menus = w.global::<Menus>();
        menus.set_titles(titles);
        menus.set_live(live.clone());
        let panel = w.global::<PanelState>();
        panel.set_film_type(film_type);
        panel.set_targets(targets as i32);
        let bar = w.global::<Bar>();
        bar.set_can_paste(live.paste_settings);
        bar.set_can_sheet(live.contact_sheet);
        bar.set_can_prepare(live.measure_roll);
        self.publish_presets();
        self.publish_rolls();
        self.publish_roll_tab();
        bar.set_filter(menus.get_titles().filter.trim_start_matches("Filter: ").to_uppercase().into());
    }

    fn wire(me: &Rc<Ctl>, window: &MainWindow) {
        Ctl::wire_canvas(me, window);
        Ctl::wire_framing(me, window);
        Ctl::wire_dedust(me, window);
        crate::editors::wire(me, window);
        let g = window.global::<StoreGlobal>();

        let p = me.clone();
        g.on_press(move |field, _fine| {
            let Some(f) = Field::from_index(field) else { return false };
            let i = f.index() as usize;
            let now = Instant::now();
            let double = {
                let mut s = p.state.borrow_mut();
                let slide = &mut s.slides[i];
                // `DSClick` reads AppKit's click count, which keeps counting past two.
                let double = slide.last_press.is_some_and(|t| now.duration_since(t) <= system::double_click());
                slide.last_press = Some(now);
                slide.resetting = double;
                slide.start = if double { None } else { Some(slide.shown) };
                if !double {
                    s.store.hold();
                    s.hands.fetch_add(1, Ordering::Relaxed);
                }
                double
            };
            if double {
                let base = f.base(p.state.borrow().store.settings());
                p.set(f, base);
            }
            double
        });

        let p = me.clone();
        g.on_drag(move |field, translation, span, fine| {
            let Some(f) = Field::from_index(field) else { return };
            let i = f.index() as usize;
            let (asked, live, offer) = {
                let mut s = p.state.borrow_mut();
                let slide = &mut s.slides[i];
                let (false, Some(start)) = (slide.resetting, slide.start) else { return };
                let asked = track::clamped(track::asked(start, translation, span, fine));
                let live = stored_at(f, asked);
                let now = Instant::now();
                slide.live = Some(live);
                slide.moved_at = Some(now);
                let offer = slide.hand.offer(now);
                if offer == Offer::Now {
                    s.moved_at = Some(now);
                }
                (asked, live, offer)
            };
            p.place(f, asked);
            if let Some(w) = p.window() {
                set_readout(&w.global::<StoreGlobal>(), f, f.spec().readout.format(live).into());
            }
            match offer {
                Offer::Now => p.set(f, live),
                Offer::After(delay) => {
                    let q = Rc::downgrade(&p);
                    p.trailing[i].start(TimerMode::SingleShot, delay, move || {
                        let Some(q) = q.upgrade() else { return };
                        let live = {
                            let mut s = q.state.borrow_mut();
                            s.slides[i].hand.trailed(Instant::now());
                            s.moved_at = s.slides[i].moved_at;
                            s.slides[i].live
                        };
                        if let Some(live) = live {
                            q.set(f, live);
                        }
                    });
                }
                Offer::Wait => {}
            }
        });

        let p = me.clone();
        g.on_release(move |field| {
            let Some(f) = Field::from_index(field) else { return };
            let i = f.index() as usize;
            let live = {
                let mut s = p.state.borrow_mut();
                let _ = s.hands.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1));
                let slide = &mut s.slides[i];
                slide.start = None;
                slide.resetting = false;
                slide.hand.released();
                slide.live.take()
            };
            p.trailing[i].stop();
            if let Some(live) = live {
                p.state.borrow_mut().store.apply(Command::Set { knob: f.knob(), value: live });
            }
            p.state.borrow_mut().store.release();
            p.publish(false);
            p.schedule_redraw();
            if p.state.borrow().last_change.is_some() {
                p.schedule_write();
            }
            // The readout leaves the hand's value for the store's, which a clamp may have moved.
            if let Some(w) = p.window() {
                let v = p.state.borrow().store.get(f);
                set_readout(&w.global::<StoreGlobal>(), f, f.spec().readout.format(v).into());
            }
        });

        // A scroll along the track writes the store at each event; one across it scrolls the panel.
        let p = me.clone();
        g.on_scroll(move |field, dx, dy, span, fine| {
            let Some(f) = Field::from_index(field) else { return false };
            if dx == 0.0 || dx.abs() <= dy.abs() {
                return false;
            }
            let from = p.state.borrow().slides[f.index() as usize].shown;
            let asked = track::clamped(track::asked(from, dx * TRACKPAD_TRAVEL, span, fine));
            p.place(f, asked);
            p.set(f, stored_at(f, asked));
            true
        });

        let p = me.clone();
        g.on_step(move |field, direction| {
            let Some(f) = Field::from_index(field) else { return };
            let (low, high) = f.spec().range;
            let v = p.state.borrow().store.get(f) + direction as f32 * (high - low) / 100.0;
            p.set(f, v);
        });

        let p = me.clone();
        g.on_reset(move |block| {
            let Some(b) = Block::from_index(block) else { return };
            let command = store::reset(p.state.borrow().store.settings(), b);
            p.write(command);
        });

        let p = me.clone();
        window.global::<Bar>().on_act(move |a| p.act(&a));
        let p = me.clone();
        window.global::<App>().on_menu(move |a| p.act(&a));
        let p = me.clone();
        window.global::<App>().on_key(move |text, control, meta, shift, option, repeat| {
            // Slint already hands ⌘ over as `control` on a Mac, and the Control key as `meta`.
            p.key(&text, control, shift, !(control || meta || shift || option), repeat)
        });
        Ctl::wire_lab(me, window);
        me.adopt_remembered_face();
        let p = me.clone();
        window.global::<Pairing>().on_close(move || p.act("close-pairing"));
        let p = me.clone();
        window.global::<crate::Notice>().on_close(move || p.neutral_decode_notice_dismissed());
        let p = me.clone();
        window.global::<PanelState>().on_auto_balance(move || p.act("auto-balance"));
        let p = me.clone();
        window.global::<PanelState>().on_pick(move |what| p.act(&what));
        let p = me.clone();
        window.global::<PanelState>().on_film_type_picked(move |i| {
            p.film_type(i);
            p.publish_bar();
        });
        let p = me.clone();
        window.global::<Confirm>().on_answered(move |go| p.answered(go));

        let lib = window.global::<Library>();
        let p = me.clone();
        lib.on_row_clicked(move |i| p.row_clicked(i.max(0) as usize));
        let p = me.clone();
        lib.on_tile_clicked(move |i, toggle, extend| p.tile_clicked(i.max(0) as usize, toggle, extend));
        let p = me.clone();
        lib.on_tile_opened(move |i| p.tile_opened(i.max(0) as usize));
        Ctl::wire_organising(me, window);
        Ctl::wire_presets(me, window);
        Ctl::wire_rolls(me, window);
        Ctl::wire_roll_tab(me, window);
        let p = me.clone();
        window.window().on_close_requested(move || p.close_requested());
        let p = me.clone();
        lib.on_scrolled(move |by| {
            {
                let mut s = p.state.borrow_mut();
                s.scroll = s.layout.clamped(s.scroll + by, s.grid.1);
            }
            let (moved, scroll) = {
                let s = p.state.borrow();
                (s.layout.cells(s.scroll, s.grid.1) != s.visible, s.scroll)
            };
            // Only a change of the visible range rebuilds the tiles; the offset alone moves them.
            if moved {
                p.publish_tiles();
            } else if let Some(w) = p.window() {
                w.global::<Library>().set_scroll(scroll);
            }
        });
        let p = me.clone();
        lib.on_resized(move |w, h| {
            let changed = {
                let mut s = p.state.borrow_mut();
                let changed = s.grid != (w, h);
                s.grid = (w, h);
                changed
            };
            if changed {
                p.relayout();
            }
        });

        let p = Rc::downgrade(me);
        me.pulse.start(TimerMode::Repeated, PULSE, move || {
            if let Some(p) = p.upgrade() {
                p.pulse();
            }
        });
    }
}

impl Ctl {
    fn wire_dedust(me: &Rc<Ctl>, window: &MainWindow) {
        let d = window.global::<crate::Dedust>();
        let p = me.clone();
        d.on_press(move |_| p.brush_press());
        let p = me.clone();
        d.on_drag(move |t, span, fine| p.brush_drag(t, span, fine));
        let p = me.clone();
        d.on_release(move || {
            p.state.borrow_mut().brush_start = None;
            p.publish_dedust();
        });
        let p = me.clone();
        d.on_remove(move |i| p.remove_correction(usize::try_from(i).unwrap_or(usize::MAX)));
        let c = window.global::<Canvas>();
        let p = me.clone();
        c.on_paint_hover(move |x, y, hover| p.paint_hover(x, y, hover));
        let p = me.clone();
        c.on_paint_start(move |x, y| p.paint_at(x, y, true));
        let p = me.clone();
        c.on_paint_move(move |x, y| p.paint_at(x, y, false));
        let p = me.clone();
        c.on_paint_end(move || p.paint_end());
    }

    fn wire_framing(me: &Rc<Ctl>, window: &MainWindow) {
        let f = window.global::<crate::Framing>();
        let ratios = [oni_settings::frame::AspectRatio::ThreeTwo, oni_settings::frame::AspectRatio::FourThree, oni_settings::frame::AspectRatio::Square, oni_settings::frame::AspectRatio::SixSeven, oni_settings::frame::AspectRatio::FiveFour];
        let p = me.clone();
        f.on_ratio_picked(move |i| {
            let ratio = usize::try_from(i - 1).ok().and_then(|k| ratios.get(k).copied());
            p.choose_framing(|g| g.ratio = ratio);
        });
        let p = me.clone();
        f.on_portrait_toggled(move |on| p.choose_framing(|g| g.portrait = on));
        let p = me.clone();
        f.on_act(move |action| p.framing_act(&action));
        let p = me.clone();
        window.global::<PanelState>().on_tab_picked(move |tab| p.tab_opened(tab));
        let c = window.global::<Canvas>();
        let p = me.clone();
        c.on_frame_grabbed(move |w, h| p.frame_grabbed(w, h));
        let p = me.clone();
        c.on_frame_dragged(move |_, _, dx, dy| p.frame_dragged(dx, dy));
        let p = me.clone();
        c.on_frame_released(move |_| p.frame_released());
    }

    /// `NegativeView`'s gestures: a drag pans, a double click toggles 100 %, the wheel zooms about
    /// the pointer, a touch surface pans unless ⌘ is held, a pinch zooms about its centre.
    fn wire_canvas(me: &Rc<Ctl>, window: &MainWindow) {
        let c = window.global::<Canvas>();
        let backing = move |p: &Rc<Ctl>| p.window().map_or(2.0, |w| f64::from(w.window().scale_factor()));

        let p = me.clone();
        c.on_resized(move || {
            let size = p.canvas_size();
            // Bound first: an `if let` scrutinee's borrow lives through its block, where the write panics.
            let full = p.state.borrow().full_size;
            if let (Some(size), Some(full)) = (size, full) {
                p.state.borrow_mut().viewport.clamp(full, size);
            }
            p.request_render();
            p.publish_framing();
        });

        let p = me.clone();
        c.on_split_dragged(move |x| {
            let width = p.window().map_or(0.0, |w| w.global::<Canvas>().get_width());
            if width <= 0.0 || p.state.borrow().split.is_none() {
                return;
            }
            let split = (x / width).clamp(0.0, 1.0);
            p.state.borrow_mut().split = Some(split);
            if let Some(w) = p.window() {
                w.global::<Canvas>().set_split(split);
            }
        });

        let p = me.clone();
        c.on_double_clicked(move || p.navigate(|v, full, size| v.toggle_hundred(full, size)));

        let p = me.clone();
        c.on_grabbed(move |x, y| p.state.borrow_mut().grab = Some((x, y)));

        let p = me.clone();
        c.on_dragged(move |x, y| {
            let Some((gx, gy)) = p.state.borrow_mut().grab.replace((x, y)) else { return };
            let b = backing(&p);
            p.navigate(|v, full, size| v.pan(viewport::pan_step(f64::from(x - gx), f64::from(y - gy), b), full, size));
        });

        let p = me.clone();
        c.on_scrolled(move |dx, dy, x, y, command| {
            if p.state.borrow().full_size.is_none() {
                return false;
            }
            let b = backing(&p);
            let wheel = viewport::take_wheel().unwrap_or(viewport::Wheel { dx: f64::from(dx), dy: f64::from(dy), precise: true });
            let at = (f64::from(x) * b, f64::from(y) * b);
            if viewport::zooms(wheel.precise, command) {
                let factor = viewport::zoom_factor(wheel.dy, wheel.precise);
                p.navigate(|v, full, size| v.zoom_by(factor, at, full, size));
            } else {
                p.navigate(|v, full, size| v.pan(viewport::pan_step(wheel.dx, wheel.dy, b), full, size));
            }
            true
        });

        let p = me.clone();
        c.on_pinch_started(move || p.state.borrow_mut().pinch = 1.0);

        let p = me.clone();
        c.on_pinched(move |scale, x, y| {
            let factor = {
                let mut s = p.state.borrow_mut();
                let factor = f64::from(scale / s.pinch.max(1e-6));
                s.pinch = scale;
                factor
            };
            let b = backing(&p);
            p.navigate(|v, full, size| v.zoom_by(factor, (f64::from(x) * b, f64::from(y) * b), full, size));
        });
    }
}

/// The tiles Slint was last given, read back by the checks.
pub fn tiles(window: &MainWindow) -> Vec<Tile> {
    let model = window.global::<Library>().get_tiles();
    (0..model.row_count()).filter_map(|i| model.row_data(i)).collect()
}

/// The display kernel's block with the screen gamut's stripes when they are on, laid by where the
/// texture sits on the canvas, so they keep one size on screen whatever the zoom.
fn zebra(s: &State, k: Params, placed: Option<Placed>, texels: u32, canvas: Option<(f64, f64)>) -> Params {
    if !s.screen_gamut {
        return k;
    }
    let started = *CLOCK.get_or_init(Instant::now);
    let height = canvas.map_or(f64::from(k.height), |c| c.1) as f32;
    let (origin, stretch) = placed.map_or(((0.0, 0.0), 1.0), |p| ((p.x as f32, p.y as f32), (p.width / f64::from(texels.max(1))) as f32));
    let phase = oni_colour::gamut::phase(started.elapsed().as_secs_f64());
    k.striped(display::Zebra { origin, stretch, canvas_height: height.max(1.0), phase })
}

fn picture_view(picture: &Picture) -> Option<wgpu::TextureView> {
    match picture {
        Picture::Resident(source) => source.view().cloned(),
        Picture::Pixels { .. } => None,
    }
}

/// The stripes' clock, as Swift reads `CACurrentMediaTime`.
static CLOCK: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

/// A frame decoded on the calling thread, the CPU's part, then laid on the GPU as its preview.
fn prepared(engine: &Shared, path: &str, reading: render::Reading) -> Result<Arc<Negative>, String> {
    let decoded = render::decode(Path::new(path), reading)?;
    let negative = engine.lock().map_err(|_| "the render engine failed earlier")?.prepare(decoded)?;
    Ok(Arc::new(negative))
}

/// The renderer's thread for thumbnails: an edit's redraw first, then the pass's next photo, each
/// waiting while a hand holds a control, so a drag never shares the GPU with the gallery.
fn paint(painter: Painter, engine: Shared, previews: Arc<Mutex<Previews>>, hands: Arc<AtomicUsize>, generation: Arc<AtomicU64>, support: PathBuf, screen: Screen) {
    enum Next {
        Redraw(String, Option<String>),
        Pass(u64, Vec<(String, String)>),
        Fill(u64, String, String),
    }
    std::thread::Builder::new()
        .name("thumbnails".into())
        .spawn(move || {
            let mut current: Option<(u64, VecDeque<(String, String)>)> = None;
            let mut seed = 0u32;
            loop {
                let next = {
                    let (lock, wake) = &*painter;
                    let mut paint = lock.lock().expect("painter");
                    loop {
                        if let Some((path, known)) = paint.redraws.pop_front() {
                            break Next::Redraw(path, known);
                        }
                        if let Some((g, rows)) = paint.pass.take() {
                            break Next::Pass(g, rows);
                        }
                        if let Some((g, queue)) = current.as_mut()
                            && let Some((path, fingerprint)) = queue.pop_front()
                        {
                            break Next::Fill(*g, path, fingerprint);
                        }
                        paint = wake.wait(paint).expect("painter");
                    }
                };
                let (pass, path, fingerprint) = match next {
                    Next::Pass(g, rows) => {
                        let (held, absent): (Vec<_>, Vec<_>) = rows.into_iter().partition(|(_, f)| thumbnails::is_held(&support, f));
                        let counts = (held.len(), absent.len());
                        current = Some((g, absent.into()));
                        with_shell(move |c| c.pass_counted(g, counts.0, counts.1));
                        continue;
                    }
                    Next::Fill(g, path, fingerprint) => {
                        if generation.load(Ordering::Relaxed) != g {
                            current = None;
                            continue;
                        }
                        (Some(g), path, fingerprint)
                    }
                    Next::Redraw(path, known) => {
                        let Some(fingerprint) = known.or_else(|| oni_library::relink::fingerprint(Path::new(&path))) else {
                            let name = stem(&path);
                            with_shell(move |c| c.thumb_landed(None, String::new(), Err("its file is unreachable".into()), name));
                            continue;
                        };
                        (None, path, fingerprint)
                    }
                };
                while hands.load(Ordering::Relaxed) > 0 {
                    std::thread::sleep(Duration::from_millis(20));
                }
                seed = seed.wrapping_add(1);
                let drawn = drawn(&engine, &previews, &path, screen, seed).and_then(|(jpeg, srgb)| {
                    thumbnails::persist(&support, &fingerprint, &jpeg).map_err(|e| e.to_string())?;
                    Ok(srgb)
                });
                let name = stem(&path);
                with_shell(move |c| c.thumb_landed(pass, fingerprint, drawn, name));
            }
        })
        .expect("thumbnail thread");
}

/// `Thumbnails.render`: the photo's own sidecar through the per-pixel pipeline at `SIDE`, as an sRGB
/// JPEG, and the tile decoded back from it so a tile drawn and a tile read are the same pixels.
fn drawn(engine: &Shared, previews: &Arc<Mutex<Previews>>, path: &str, screen: Screen, seed: u32) -> Result<(Vec<u8>, (Vec<u8>, usize, usize)), String> {
    let asked = Instant::now();
    let mosaic = oni_decode::decodes_via_mosaic(Path::new(path));
    let settings = oni_settings::sidecar::read(Path::new(path), mosaic, true).map_err(|e| e.to_string())?.settings;
    let known = previews.lock().expect("previews").get(path);
    let negative = match known {
        Some(n) => n,
        None => {
            let n = prepared(engine, path, render::Reading::of(&settings))?;
            previews.lock().expect("previews").insert(path.to_string(), n.clone());
            n
        }
    };
    let mut guard = engine.lock().map_err(|_| "the render engine failed earlier")?;
    let started = Instant::now();
    let linear = guard.render(&negative, &settings.per_pixel_only(), Some(thumbnails::SIDE))?;
    let pipeline = started.elapsed();
    drop(guard);
    let (w, h) = (linear.width, linear.height);
    let srgb = render::encoded(&linear, &Params::new(Screen::Srgb, w as u32, h as u32, seed), 1);
    let jpeg = thumbnails::jpeg(&srgb, w, h).ok_or("the thumbnail cannot be encoded")?;
    let tile = thumbnails::decoded(&jpeg, screen).ok_or("the thumbnail cannot be read back")?;
    timing::render(Kind::Thumbnail, &stem(path), pipeline, asked.elapsed());
    Ok((jpeg, tile))
}

/// `Thumbnails.drain`: the folder's files read off this thread, the latest asked first, so a
/// scroll shows the cells under the eye before those it left.
fn read_disk(support: PathBuf, screen: Screen) -> mpsc::Sender<String> {
    let (tx, rx) = mpsc::channel::<String>();
    std::thread::Builder::new()
        .name("thumbnail files".into())
        .spawn(move || {
            let mut stack: Vec<String> = Vec::new();
            loop {
                if stack.is_empty() {
                    match rx.recv() {
                        Ok(f) => stack.push(f),
                        Err(_) => return,
                    }
                }
                stack.extend(rx.try_iter());
                let Some(fingerprint) = stack.pop() else { continue };
                let read = thumbnails::load(&support, &fingerprint, screen);
                with_shell(move |c| c.disk_landed(fingerprint, read));
            }
        })
        .expect("thumbnail files thread");
    tx
}
