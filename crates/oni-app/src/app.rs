//! The window's controller: the state and the types every part shares. Each part of the window is
//! driven from its own file under `app/`, named for what it does (`rendering`, `menus`, `keys`…).

use crate::display::{self, Params, Screen};
use crate::fields::{Block, Field};
use crate::gallery::{self, Layout, Metrics};
use crate::library::{self, Archive};
use crate::preferences::{self, Preferences};
use crate::thumbnails::{self, Memory};
use crate::render::{self, Delivery, InputHistograms, Job, Negative, Placed, Previews, Shared, View, Worker};
use crate::viewport::{self, Viewport};
use crate::framing::{self as frames, Handle};
use oni_settings::{cg::Rect as Fraction, frame::Geometry, geometry::{self as geom, Framing as Remembered}};
use crate::phone::{self, Phone};
use crate::store::{self, Store as Settings};
use crate::timing::{self, Kind, Notches};
use crate::{Act, App, Bar, Canvas, Confirm, Heading, Mark, Library, MainWindow, MenuLive, MenuTitles, Menus, Pairing, PanelState, Plots, Store as StoreGlobal, Tile, TreeRow};
use oni_core::{Command, PasteCheck, Surface};
use oni_settings::ConversionMode;
use crate::link::Outward;
use oni_link::HostEvent;
use crate::motion::Spring;
use crate::system;
use oni_i18n::{tr, trn};
use crate::track::{self, Hand, Offer};
use slint::{ComponentHandle, Image, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, Timer, TimerMode, VecModel};
use std::cell::RefCell;
use oni_library::{gallery_history::Move, index::LibraryIndex, roots::Survey};
use oni_settings::Medal;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::time::{Duration, Instant};

pub mod export_card;
mod publishing;
mod cropping;
mod dedusting;
mod rendering;
mod archive_view;
mod gallery_view;
mod phone_link;
mod settings_sheet;
mod frontier_look;
mod actions;
mod keys;
mod gestures;
mod route;
mod menus;
mod wiring;
mod workers;
mod shell;
mod canvas_gpu;
mod opening;
use crate::slider_track::{position, position_on, stored_at, stored_on, track_on};
mod sidebar_tree;
mod tree_column;
mod project_drop;
mod tree_reveal;
mod tree_clicks;
mod state_read;
use state_read::LoadBar;
pub use workers::tiles;
use workers::{paint, prepared, prepared_keeping, read_disk, zebra};

const FRAME_TICK: Duration = Duration::from_millis(16);
/// What changes on its own rhythm, histogram and load counter, is published at this pace.
const PULSE: Duration = Duration::from_millis(100);
/// The least a picture waits for its count, so the event loop runs between two counts.
const MIN_WAIT: Duration = Duration::from_millis(2);
/// `DS.Control.cropHint` and `DS.Space.md`: the crop badge's margin inside the view and its rise above the frame.
const BADGE_MARGIN: f64 = 32.0;
const BADGE_RISE: f64 = 16.0;
/// `DSScroll.idleWindow`: a pause this long ends a straightening gesture.
const STRAIGHTEN_IDLE: Duration = Duration::from_millis(160);
/// `Sidecar.writeDelay`: the hand rests this long before the sidecar is written.
pub const WRITE_DELAY: Duration = oni_settings::sidecar::WRITE_DELAY;

mod board;
mod folds;
mod lab;
mod loupe;
mod masks;
mod navigator;
mod organising;
mod help;
mod prepare;
mod presets;
mod roll_tab;
mod rolls;
mod sheets;
mod stock_picker;
pub(crate) use stock_picker::StockTarget;
mod transition;
mod updates;
mod year;
mod scroll;
mod tree_rename;
/// The folder picker `Open Folder…` shows; a check hands its own.
pub type Picker = Box<dyn Fn() -> Option<PathBuf>>;
pub use help::Reporter;
pub use updates::{Fetch, Reveal, Updater};

/// Where a deleted preset file goes: the system's Trash, or a check's own folder.
pub type Discard = Box<dyn Fn(&Path) -> std::io::Result<()>>;
/// Where the Folder key shows a folder: the Finder, or a check's own record.
pub type Shower = Box<dyn Fn(&Path)>;

pub struct Options {
    pub screen: Screen,
    /// Slint draws the window on the engine's device and takes the canvas texture as it is; on its
    /// software renderer the canvas is read back.
    pub canvas: bool,
    /// `AppFolders.support`: the phone link's keys, the preferences, the thumbnails.
    pub support: PathBuf,
    /// `AppFolders.caches`: the index, which the system may purge.
    pub caches: PathBuf,
    pub picker: Picker,
    pub discard: Discard,
    pub reporter: Reporter,
    /// `FlatFieldBlock.load`'s open panel and the Folder key's Finder.
    pub mask_picker: Picker,
    pub show_folder: Shower,
    /// The home folder and the volumes' folder the navigator walks; a check hands a disk of its own.
    pub computer: oni_library::navigator::Computer,
    /// `requireRoll`: a frame never measured opens on Prepare the Roll; off as under Swift's probes,
    /// so a check opens bare frames by hand, and on in the app itself.
    pub require_roll: bool,
    /// The update feed's reader; none looks nowhere, as the checks run.
    pub updates: Option<Updater>,
    /// `Propagation.board`: the general pasteboard in the app, a board of the process's own for a check.
    pub pasteboard: Box<dyn crate::pasteboard::Board>,
    /// Apple's decode, which turns the transition from the earlier version on; none in the checks.
    pub transition: Option<Arc<dyn oni_transition::AppleDecode>>,
    /// Names the volume a path lies on: the system's in the app, a check's own for its folders.
    pub identify: oni_library::bookmarks::Identify,
}

/// `EditorScene.open`'s panel message.
pub const PICKER_MESSAGE: &str = oni_i18n::marked("Choose a folder: every photo under it shows in the archive. Nothing is copied or moved.");

impl Options {
    /// The bug reporter the app ships with, posting to the site.
    pub fn online_reporter() -> help::Reporter {
        std::sync::Arc::new(help::post)
    }
}

impl Default for Options {
    fn default() -> Self {
        Options {
            screen: Screen::Srgb,
            canvas: false,
            // A check's own folder and no reporter: it never writes the owner's journal nor opens an issue.
            support: std::env::temp_dir().join(format!("oni-support-{}", std::process::id())),
            caches: std::env::temp_dir().join(format!("oni-caches-{}", std::process::id())),
            picker: Box::new(|| system::pick_folder(tr(PICKER_MESSAGE))),
            discard: Box::new(system::trash),
            reporter: std::sync::Arc::new(|_: &str, _: Option<String>| Err("no reporter in a check".to_string())),
            mask_picker: Box::new(|| system::pick_file(masks::LOAD_MESSAGE, oni_library::library::IMAGE_EXTENSIONS)),
            show_folder: Box::new(system::show_folder),
            // The app hands this machine's; a check gets none unless it hands its own.
            computer: oni_library::navigator::Computer::none(),
            require_roll: false,
            updates: None,
            pasteboard: Box::new(crate::pasteboard::Memory::default()),
            transition: None,
            identify: oni_library::bookmarks::system(),
        }
    }
}

/// The thumbnails pass and the edits' redraws, one render at a time for the app, a redraw first.
#[derive(Default)]
struct Paint {
    /// An edit's redraws by path, with the fingerprint the index knows.
    redraws: VecDeque<(String, Option<String>)>,
    /// The pass a folder asked last, by generation, and the thumbnails its unreachable photos need.
    pass: Option<(u64, Vec<(String, String)>, Vec<String>)>,
}

type Painter = Arc<(Mutex<Paint>, Condvar)>;

/// `pasteSettings`'s dialog when a look crosses rolls.
const ANOTHER_ROLL: (&str, &str) = (oni_i18n::marked("Another Roll"), oni_i18n::marked("You are trying to apply settings from one roll to another: results may vary."));

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
    DownloadUpdate,
    Locate(navigator::Asked),
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
    current: Option<oni_graph::Source>,
    /// The window cannot take the GPU's textures (Slint's software renderer): the canvas is read back.
    reads_back: bool,
    /// The whole picture the histogram counts next, at one size whatever the zoom; none once counted.
    uncounted: Option<oni_graph::Source>,
    seed: u32,
    screen: display::Screen,
    /// When the last count started, which holds the counts to the image's cadence.
    counted_at: Option<Instant>,
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
    survey: Arc<Survey>,
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
    /// The archive's and the projects' trees as laid, and the photo the column last centred on.
    tree: tree_column::Laid,
    project_tree: tree_column::Laid,
    tree_centred: Option<String>,
    reveal: tree_reveal::Reveal,
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
    histogram: Option<oni_measure::histogram::Histogram>,
    histogram_dirty: bool,
    last_change: Option<Instant>,
    /// The store revision `publish` last saw, so any edit of the open frame arms its write.
    published_revision: u64,
    /// How the open frame's preview was decoded: a sidecar naming another reading decodes it again.
    reading: Option<render::Reading>,
    /// Where the stock picker's choice lands while it shows.
    stock_target: Option<StockTarget>,
    /// The window may close: a failed write was answered with Quit Anyway.
    quitting: bool,
    /// The flat-field masks loaded by hand, read once and again after each change to them.
    masks: Vec<crate::flat_fields::Entry>,
    masking: masks::Masking,
    status: String,
    brush: f32,
    gpu: Gpu,
    renders: usize,
    /// How many times a photo was opened, which the checks count.
    opens: usize,
    opened_at: Instant,
    notches: Notches,
    /// When the hand moved the value the next render shows; `None` times a render from its ask.
    moved_at: Option<Instant>,
    viewport: Viewport,
    views: Notches,
    /// The open frame's full-resolution pixels once built, and the frame one is being built for.
    full: Option<(String, oni_graph::Source)>,
    building_full: Option<String>,
    /// The frames whose full-resolution decode failed under their current reading.
    full_failed: HashSet<String>,
    loupe: loupe::Held,
    /// What the canvas asked of the open frame and was given, reset whole for another frame.
    requests: rendering::Requests,
    dust: dedusting::Dust,
    compare: rendering::Compare,
    /// The frames edited together, when more than one is.
    together: Option<gallery_view::together::Group>,
    /// The displayed image's size in full-resolution pixels, from the last render placed.
    full_size: Option<(f64, f64)>,
    placed: Option<Placed>,
    grab: Option<(f32, f32)>,
    pinch: f32,
    /// The ratio and orientation last chosen, and the frame chosen on, handed to the next frame framed.
    framing_memory: Option<(Remembered, String)>,
    /// A frame held on the canvas: the window's or the crop's, the handle, the frame at the press
    /// and the shown image frame's size in points.
    frame_gesture: Option<(bool, Handle, Fraction, (f64, f64))>,
    /// Esc put the held frame back: the rest of that drag moves nothing.
    frame_cancelled: bool,
    /// A scroll along a track under way, its hand raised until the idle window passes.
    scrolling: bool,
    /// The frame a straightening gesture set out from, the one it last wrote, and when.
    straightening: Option<(Geometry, Geometry, Instant)>,
    /// The tab shown, the last panel the hand was in and the measuring window armed.
    route: crate::tabs::Route,
    /// The stroke being painted, in fractions of the rendered frame, y down.
    painting_stroke: dedusting::live::Live,
    brush_start: Option<f32>,
    brush_pressed_at: Option<Instant>,
    /// `Propagation`'s copied measuring window.
    window_board: Option<Fraction>,
    /// `renameRequest`: the folder New Roll made, named in place once its row shows, and the row being named.
    rename_request: Option<String>,
    tree_renaming: Option<String>,
    pasteboard: Box<dyn crate::pasteboard::Board>,
    /// The board's change count last read, so a menu publication reads only types that moved.
    board_seen: Option<i64>,
    /// `screenGamutWarning`: the pixels this screen cannot show, striped on the canvas.
    screen_gamut: bool,
    support: PathBuf,
    phone: Option<Phone>,
    picker: Picker,
    discard: Discard,
    reporter: Reporter,
    /// The notes read this session, which keep their pill until the app quits.
    news_kept: HashSet<String>,
    /// Bumped at each opening of Report a Bug, so a post answering a sheet closed since lands nowhere.
    report_round: u64,
    /// The console window while it is open, and the timer that reads the log into it.
    console: Option<(crate::ConsoleWindow, Timer)>,
    board: Option<crate::Board>,
    nav: navigator::Nav,
    pub(crate) prepare: prepare::Roll,
    pub(crate) making_sheet: Option<sheets::Making>,
    pub(crate) updates: updates::Updates,
    year: year::Year,
    transition: transition::Transition,
}

pub struct Ctl {
    window: slint::Weak<MainWindow>,
    pub state: RefCell<State>,
    engine: Shared,
    worker: Worker,
    trailing: Vec<Timer>,
    arrivals: Vec<Timer>,
    pulse: Timer,
    /// Polls the histogram's readback while one is under way, so it lands within a frame.
    histogram_watch: Timer,
    settle: Timer,
    stripes: Timer,
    scroll_idle: Timer,
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

fn image(bytes: &[u8], w: usize, h: usize) -> Image {
    Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(bytes, w as u32, h as u32))
}

use oni_library::paths::stem;

pub struct Shell {
    pub window: MainWindow,
    pub ctl: Rc<Ctl>,
}

impl Ctl {
    pub(crate) fn window(&self) -> Option<MainWindow> {
        self.window.upgrade()
    }
}
