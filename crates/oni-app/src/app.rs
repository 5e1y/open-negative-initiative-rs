//! The window and what drives it: the sliders' timing, the store's publishing, the canvas's render
//! loop, the gallery's visible tiles, the tree, the bar, the menus and the keys.

use crate::display::{self, Params, Screen};
use crate::fields::{Block, Field};
use crate::gallery::{self, Layout, Metrics};
use crate::library::{self, Archive};
use crate::render::{self, Done, Histogram, Job, Negative, Picture, Placed, Previews, Shared, View, Worker};
use crate::viewport::{self, Viewport};
use crate::phone::{self, Phone};
use crate::store::{self, Store as Settings};
use crate::timing::{self, Kind, Notches};
use crate::{App, Bar, Canvas, Heading, Library, MainWindow, Pairing, PanelState, Plots, Store as StoreGlobal, Tile, TreeRow};
use oni_core::Command;
use oni_core::link::Outward;
use oni_link::HostEvent;
use crate::motion::Spring;
use crate::track::{self, DOUBLE_CLICK, Hand, Offer, TRACKPAD_TRAVEL};
use slint::{ComponentHandle, Image, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, Timer, TimerMode, VecModel};
use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Long side of a gallery thumbnail, in pixels: a 6-column tile at 2x on a wide window.
pub const THUMB_SIDE: usize = 480;
const FRAME_TICK: Duration = Duration::from_millis(16);
/// What changes on its own rhythm, histogram and load counter, is published at this pace.
const PULSE: Duration = Duration::from_millis(100);
/// `sidecar.writeDelay`: the hand rests this long before the sidecar is written.
const WRITE_DELAY: Duration = Duration::from_millis(500);
/// `DSHistogram`'s raster: the panel's width at 2x by the plot's height at 2x.
const HISTOGRAM: (usize, usize) = (680, 300);

pub struct Options {
    pub screen: Screen,
    /// The device Slint draws the window with, on which the display kernel writes the canvas;
    /// none on the software renderer.
    pub canvas: Option<(wgpu::Device, wgpu::Queue)>,
    /// Where the phone link keeps its key and the phones it trusts.
    pub support: PathBuf,
}

impl Default for Options {
    fn default() -> Self {
        Options { screen: Screen::Srgb, canvas: None, support: timing::support() }
    }
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
    store: Box<dyn Settings>,
    screen: Screen,
    archive: Option<Archive>,
    /// Every photo's path in gallery order.
    order: Vec<String>,
    open: Option<usize>,
    selection: BTreeSet<usize>,
    anchor: Option<usize>,
    columns: usize,
    scroll: f32,
    grid: (f32, f32),
    layout: Layout,
    visible: std::ops::Range<usize>,
    thumbs: HashMap<String, Image>,
    loaded: usize,
    /// The open frame's preview, which the canvas renders; the previews kept besides it.
    negative: Option<(String, Arc<Negative>)>,
    previews: Arc<Mutex<Previews>>,
    /// `DSGesture`: hands on a control, which hold the thumbnails' decodes back.
    hands: Arc<AtomicUsize>,
    slides: Vec<Slide>,
    rendered: u64,
    requested: u64,
    histogram: Option<Histogram>,
    histogram_dirty: bool,
    last_change: Option<Instant>,
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
    requested_view: Option<(Viewport, (f64, f64), bool, bool, bool)>,
    placed: Option<Placed>,
    grab: Option<(f32, f32)>,
    pinch: f32,
    /// Compare Before/After's split, a fraction of the canvas's width; none while off.
    split: Option<f32>,
    /// `screenGamutWarning`: the pixels this screen cannot show, striped on the canvas.
    screen_gamut: bool,
    before_placed: Option<Placed>,
    /// How many distinct befores the canvas encoded, which a kept one does not add to.
    befores: usize,
    support: PathBuf,
    phone: Option<Phone>,
}

impl State {
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
        self.open.and_then(|i| self.order.get(i)).map(String::as_str)
    }

    pub fn loaded(&self) -> usize {
        self.loaded
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
        let state = State {
            store,
            screen: options.screen,
            archive: None,
            order: Vec::new(),
            open: None,
            selection: BTreeSet::new(),
            anchor: None,
            columns: gallery::COLUMN_STEPS[1],
            scroll: 0.0,
            grid: (0.0, 0.0),
            layout: Layout::new(&[], 6, 0.0, Metrics::GALLERY),
            visible: 0..0,
            thumbs: HashMap::new(),
            loaded: 0,
            negative: None,
            previews: Arc::default(),
            hands: Arc::default(),
            slides: Field::ALL.iter().map(|_| Slide::default()).collect(),
            rendered: 0,
            requested: 0,
            histogram: None,
            histogram_dirty: false,
            last_change: None,
            status: String::new(),
            brush: 0.0,
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
            placed: None,
            grab: None,
            pinch: 1.0,
            split: None,
            befores: 0,
            screen_gamut: false,
            before_placed: None,
            support: options.support.clone(),
            phone: None,
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
        });
        SHELL.with(|s| *s.borrow_mut() = Rc::downgrade(&ctl));
        Ctl::wire(&ctl, &window);
        ctl.publish(true);
        ctl.publish_bar();
        // A phone paired before finds the desktop listening again; nothing pairs until asked.
        if phone::has_trusted(&options.support) {
            ctl.start_phone();
        }
        Ok(Shell { window, ctl })
    }

    /// Opens a folder: its tree and gallery at once, thumbnails decoded behind, the first photo open.
    pub fn open_folder(&self, folder: &Path) {
        self.ctl.open_folder(folder);
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
    fn publish(self: &Rc<Self>, jump: bool) {
        let Some(w) = self.window() else { return };
        let g = w.global::<StoreGlobal>();
        let moved = {
            let mut s = self.state.borrow_mut();
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
            out
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
        if !moved.is_empty() || jump {
            self.state.borrow_mut().last_change = Some(Instant::now());
        }
        self.refresh_phone();
        self.publish_bar();
        self.request_render();
    }

    /// Every property is read before any is set, so no Slint callback runs under the state's borrow.
    fn publish_bar(&self) {
        let Some(w) = self.window() else { return };
        let bar = w.global::<Bar>();
        let (has_photo, undo, redo, selected) = {
            let s = self.state.borrow();
            (s.open.is_some(), s.store.can_undo(), s.store.can_redo(), s.selection.len())
        };
        bar.set_has_photo(has_photo);
        bar.set_can_undo(has_photo && undo);
        bar.set_can_redo(has_photo && redo);
        bar.set_can_reset(has_photo);
        bar.set_targets(selected.max(usize::from(has_photo)) as i32);
        bar.set_editing(!w.global::<App>().get_gallery());
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

    fn set(self: &Rc<Self>, f: Field, stored: f32) {
        self.write(Command::Set { knob: f.knob(), value: stored });
    }

    // MARK: - Rendering

    /// Lays the open frame's current settings in the render slot, when they differ from the last asked.
    fn request_render(self: &Rc<Self>) {
        let size = self.canvas_size();
        let (job, build, heal) = {
            let mut s = self.state.borrow_mut();
            let Some(path) = s.open_path().map(str::to_string) else { return };
            let revision = s.store.revision();
            let moved_at = s.moved_at.take();
            let settled = s.navigated_at.is_none_or(|t| t.elapsed() >= viewport::SETTLE);
            let full = s.full.as_ref().filter(|(p, _)| *p == path).map(|(_, f)| f.clone());
            let view_key = size.map(|size| (s.viewport, size, settled, full.is_some(), s.split.is_some()));
            let moved = revision != s.requested || s.rendered == 0;
            if !moved && view_key == s.requested_view {
                return;
            }
            let Some(negative) = s.negative.as_ref().filter(|(p, _)| *p == path).map(|(_, n)| n.clone()) else { return };
            s.requested = revision;
            s.requested_view = view_key;
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
                (path.clone(), s.store.settings().primaries_converted)
            });
            let view = size.map(|size| View { viewport: s.viewport, size, full, settled });
            let live = s.store.settings().clone();
            // The cache is purged at every quit, so each correction's patch is generated as its frame opens.
            let caches = crate::patches::caches();
            let unhealed = live.corrections.iter().any(|c| !c.stroke.is_empty() && !crate::patches::file(&caches, c.id).exists());
            let heal = (unhealed && s.healing.as_deref() != Some(path.as_str())).then(|| {
                s.healing = Some(path.clone());
                (path.clone(), live.clone())
            });
            // `NegativeView.referenceSettings`: the bar's reset frame, the dust still healed.
            let reference = s.split.map(|_| {
                let mut reference = oni_core::parameters::imported(&live);
                reference.corrections = live.corrections.clone();
                reference
            });
            (Job { revision, path: PathBuf::from(path), negative, settings: live, kind, asked, view, reference }, build, heal)
        };
        self.worker.submit(job);
        if let Some((path, converted)) = build {
            self.build_full(path, converted);
        }
        if let Some((path, settings)) = heal {
            self.heal(path, settings);
        }
    }

    /// `ensureGenerated` for each correction of the open frame that has no patch yet, one after the
    /// other on the full-resolution frame, so each source holds the patches before it healed.
    fn heal(self: &Rc<Self>, path: String, settings: oni_settings::PipelineSettings) {
        let engine = self.engine.clone();
        std::thread::spawn(move || {
            let caches = crate::patches::caches();
            let made = (|| -> Result<usize, String> {
                let decoded = render::decode_full(Path::new(&path), settings.primaries_converted)?;
                let full = engine.lock()?.full(decoded)?;
                let mut made = 0;
                for c in &settings.corrections {
                    if c.stroke.is_empty() || crate::patches::file(&caches, c.id).exists() {
                        continue;
                    }
                    let (region, size) = engine.lock()?.search_region(&full, &settings, c)?;
                    let patch = crate::heal::generate(&region, c, size.0, size.1);
                    crate::patches::write(&caches, c.id, patch.width as u32, patch.height as u32, &patch.pixels)?;
                    engine.lock()?.forget_patch(c.id);
                    made += 1;
                }
                Ok(made)
            })();
            with_shell(move |c| {
                c.state.borrow_mut().healing = None;
                match made {
                    Ok(0) => {}
                    Ok(n) => {
                        timing::line(&format!("{}: {n} dust patches generated", stem(&path)));
                        if c.state.borrow().open_path() == Some(path.as_str()) {
                            c.state.borrow_mut().requested_view = None;
                            c.request_render();
                        }
                    }
                    Err(e) => timing::line(&format!("{}: dust patches not generated: {e}", stem(&path))),
                }
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
    fn build_full(self: &Rc<Self>, path: String, converted: bool) {
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
        self.settle.start(TimerMode::SingleShot, viewport::SETTLE, move || {
            if let Some(me) = me.upgrade() {
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
            s.gpu.is_some()
        };
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
            match (s.placed, self.canvas_size()) {
                (Some(p), Some(size)) => format!("{name} · {}", viewport::zoom_label(s.viewport.effective_scale(p.full, size), p.at_full_resolution)),
                _ => format!("{name} · {width} × {height}"),
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
        let (histogram, loading, flush) = {
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
            let total = s.order.len();
            let loading = (s.loaded < total).then(|| format!("Reading thumbnails · {} of {total}", s.loaded));
            let flush = s.last_change.is_some_and(|t| t.elapsed() >= WRITE_DELAY) && s.slides.iter().all(|sl| sl.live.is_none());
            (histogram, loading, flush)
        };
        if let Some(h) = histogram {
            let (pw, ph) = HISTOGRAM;
            w.global::<Plots>().set_output(premultiplied(&h.raster(pw, ph), pw, ph));
        }
        if let Some(text) = loading
            && w.global::<App>().get_gallery()
        {
            w.global::<Canvas>().set_status(text.into());
        }
        if flush {
            let failure = {
                let mut s = self.state.borrow_mut();
                s.last_change = None;
                s.store.flush()
            };
            if let Some(message) = failure {
                self.say(&message);
            }
        }
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

    fn open_folder(self: &Rc<Self>, folder: &Path) {
        let archive = Archive::load(folder);
        let order: Vec<String> = archive.entries().iter().map(|e| e.path.clone()).collect();
        let (title, stock) = archive.groups.first().map(|g| (g.title.clone(), g.stock.clone())).unwrap_or_default();
        {
            let mut s = self.state.borrow_mut();
            s.order = order.clone();
            s.store.set_archive(order.clone());
            s.archive = Some(archive);
            s.selection.clear();
            s.loaded = 0;
            s.thumbs.clear();
        }
        if let Some(w) = self.window() {
            let lib = w.global::<Library>();
            lib.set_roll_title(title.into());
            lib.set_stock(stock.into());
        }
        self.relayout();
        self.publish_tree();
        self.read_thumbnails(order.clone());
        if let Some(first) = order.first() {
            self.open_photo(first);
        }
    }

    fn publish_tree(&self) {
        let Some(w) = self.window() else { return };
        let s = self.state.borrow();
        let Some(archive) = &s.archive else { return };
        let open = s.open_path();
        let rows: Vec<TreeRow> = archive
            .rows()
            .into_iter()
            .map(|r| TreeRow {
                depth: r.depth as i32,
                title: r.title.into(),
                caption: r.caption.into(),
                kind: r.kind,
                expandable: r.expandable,
                expanded: r.expanded,
                selected: matches!(&r.id, oni_library::archive_tree::NodeId::Photo(p) if Some(p.as_str()) == open),
            })
            .collect();
        drop(s);
        w.global::<Library>().set_rows(ModelRc::new(VecModel::from(rows)));
    }

    fn row_clicked(self: &Rc<Self>, i: usize) {
        let path = {
            let mut s = self.state.borrow_mut();
            let Some(archive) = s.archive.as_mut() else { return };
            let Some(row) = archive.rows().get(i).cloned() else { return };
            archive.toggle(&row.id)
        };
        match path {
            Some(path) => {
                // Bound first: an `if let` scrutinee's borrow lives through its block.
                let index = self.state.borrow().order.iter().position(|p| *p == path);
                if let Some(index) = index {
                    let mut s = self.state.borrow_mut();
                    s.selection = BTreeSet::from([index]);
                    s.anchor = Some(index);
                }
                self.open_photo(&path);
                self.publish_tiles();
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
            s.open = s.order.iter().position(|p| p == path);
            s.opened_at = Instant::now();
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
        let (previews, converted) = {
            let s = self.state.borrow();
            (s.previews.clone(), s.store.settings().primaries_converted)
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

    /// Every photo's thumbnail through the pipeline on its own sidecar, off this thread, one by one.
    fn read_thumbnails(&self, order: Vec<String>) {
        let (previews, hands, screen) = {
            let s = self.state.borrow();
            (s.previews.clone(), s.hands.clone(), s.screen)
        };
        let engine = self.engine.clone();
        std::thread::Builder::new()
            .name("thumbnails".into())
            .spawn(move || {
                for (n, path) in order.into_iter().enumerate() {
                    while hands.load(Ordering::Relaxed) > 0 {
                        std::thread::sleep(Duration::from_millis(20));
                    }
                    let asked = Instant::now();
                    let thumbnail = (|| -> Result<(Vec<u8>, usize, usize), String> {
                        let mosaic = oni_decode::decodes_via_mosaic(Path::new(&path));
                        let settings = oni_settings::sidecar::read(Path::new(&path), mosaic, true).map_err(|e| e.to_string())?.settings;
                        let known = previews.lock().expect("previews").get(&path);
                        let negative = match known {
                            Some(n) => n,
                            None => {
                                let n = prepared(&engine, &path, settings.primaries_converted)?;
                                previews.lock().expect("previews").insert(path.clone(), n.clone());
                                n
                            }
                        };
                        let mut engine = engine.lock().map_err(|_| "the render engine failed earlier")?;
                        let started = Instant::now();
                        let linear = engine.render(&negative, &settings, Some(THUMB_SIDE))?;
                        let pipeline = started.elapsed();
                        drop(engine);
                        let k = Params::new(screen, linear.width as u32, linear.height as u32, n as u32);
                        let bytes = render::encoded(&linear, &k, 1);
                        timing::render(Kind::Thumbnail, &stem(&path), pipeline, asked.elapsed());
                        Ok((bytes, linear.width, linear.height))
                    })();
                    // A frame that fails still counts as read, its tile left empty and the reason said.
                    with_shell(move |c| {
                        let failure = {
                            let mut s = c.state.borrow_mut();
                            s.loaded += 1;
                            match thumbnail {
                                Ok((bytes, w, h)) => {
                                    s.thumbs.insert(path, image(&bytes, w, h));
                                    None
                                }
                                Err(e) => Some(format!("{}: {e}", stem(&path))),
                            }
                        };
                        if let Some(message) = failure {
                            c.say(&message);
                        }
                        c.publish_tiles();
                    });
                }
            })
            .expect("thumbnail thread");
    }

    // MARK: - The gallery

    fn relayout(&self) {
        {
            let mut s = self.state.borrow_mut();
            let groups: Vec<(String, String, usize)> = s
                .archive
                .as_ref()
                .map(|a| a.groups.iter().map(|g| (g.title.clone(), g.caption.clone(), g.entries.len())).collect())
                .unwrap_or_default();
            s.layout = Layout::new(&groups, s.columns, s.grid.0, Metrics::GALLERY);
            s.scroll = s.layout.clamped(s.scroll, s.grid.1);
        }
        self.publish_tiles();
    }

    /// The cells in view and one row either side, the only tiles Slint is given.
    fn publish_tiles(&self) {
        let Some(w) = self.window() else { return };
        let lib = w.global::<Library>();
        let mut s = self.state.borrow_mut();
        let range = s.layout.cells(s.scroll, s.grid.1);
        let labels: HashMap<String, i32> = s
            .archive
            .as_ref()
            .map(|a| a.entries().iter().map(|e| (e.path.clone(), library::medal_index(e.label))).collect())
            .unwrap_or_default();
        let tiles: Vec<Tile> = range
            .clone()
            .filter_map(|i| {
                let frame = s.layout.frame(i)?;
                let path = s.order.get(i)?;
                Some(Tile {
                    index: i as i32,
                    x: frame.x,
                    y: frame.y,
                    width: frame.width,
                    height: s.layout.tile.1,
                    picture: s.thumbs.get(path).cloned().unwrap_or_default(),
                    caption: stem(path).into(),
                    selected: s.selection.contains(&i),
                    medal: labels.get(path).copied().unwrap_or(0),
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
        {
            let mut s = self.state.borrow_mut();
            match (toggle, extend, s.anchor) {
                (_, true, Some(anchor)) => s.selection = (anchor.min(i)..=anchor.max(i)).collect(),
                (true, _, _) => {
                    if !s.selection.remove(&i) {
                        s.selection.insert(i);
                    }
                    s.anchor = Some(i);
                }
                _ => {
                    s.selection = BTreeSet::from([i]);
                    s.anchor = Some(i);
                }
            }
        }
        self.publish_tiles();
        self.publish_bar();
    }

    fn tile_opened(self: &Rc<Self>, i: usize) {
        let path = self.state.borrow().order.get(i).cloned();
        if let Some(path) = path {
            self.open_photo(&path);
            if let Some(w) = self.window() {
                w.global::<App>().set_gallery(false);
            }
            self.publish_bar();
        }
    }

    /// Labels the gallery's selection, or the open photo in the editor, on each sidecar.
    fn label(self: &Rc<Self>, medal: i32) {
        let gallery = self.window().is_some_and(|w| w.global::<App>().get_gallery());
        let targets: Vec<String> = {
            let s = self.state.borrow();
            if gallery {
                s.selection.iter().filter_map(|i| s.order.get(*i).cloned()).collect()
            } else {
                s.open_path().map(str::to_string).into_iter().collect()
            }
        };
        let label = library::medal_of(medal);
        for path in &targets {
            match oni_settings::sidecar::set_label(label, Path::new(path), true) {
                Ok(()) => {
                    if let Some(a) = self.state.borrow_mut().archive.as_mut() {
                        a.set_label(path, label);
                    }
                }
                Err(e) => self.say(&e.to_string()),
            }
        }
        self.publish_tiles();
        self.publish_tree();
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
        let index = self.state.borrow().order.iter().position(|p| *p == path);
        if let Some(index) = index {
            let mut s = self.state.borrow_mut();
            s.selection = BTreeSet::from([index]);
            s.anchor = Some(index);
        }
        self.open_photo(&path);
        self.publish_tiles();
    }

    // MARK: - Gestures

    /// Bar keys, menu items and shortcuts meet here; what this build does not carry says so.
    pub fn act(self: &Rc<Self>, action: &str) {
        let Some(w) = self.window() else { return };
        let app = w.global::<App>();
        match action {
            "undo" => self.write(Command::Undo),
            "redo" => self.write(Command::Redo),
            "reset" => self.write(Command::Reset),
            "toggle-mode" => {
                let gallery = !app.get_gallery();
                if !gallery && self.state.borrow().open.is_none() {
                    return;
                }
                app.set_gallery(gallery);
                // A mode change drops the split, as Swift's does.
                if self.state.borrow().split.is_some() {
                    self.toggle_compare();
                }
                self.publish_bar();
                self.publish_tiles();
            }
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
            "screen-gamut" => self.toggle_screen_gamut(),
            "bigger-brush" | "smaller-brush" => {
                let by = if action == "bigger-brush" { 1.0 } else { -1.0 };
                let brush = {
                    let mut s = self.state.borrow_mut();
                    s.brush += by;
                    s.brush
                };
                self.say(&format!("Brush step {brush:+}: the De-dust brush is not in this build yet"));
            }
            "pair-phone" => self.pair_phone(),
            "close-pairing" => self.close_pairing(),
            a if a.starts_with("medal-") => {
                if let Ok(m) = a[6..].parse::<i32>() {
                    self.label(m);
                }
            }
            other => self.say(&format!("“{other}” is not in this build yet")),
        }
    }

    /// The key monitor: modifier chords always, bare keys only when no field holds the keyboard, which
    /// is given: a text field consumes its printable keys before they reach this scope.
    fn key(self: &Rc<Self>, text: &str, command: bool, shift: bool, _option: bool) -> bool {
        if text == "\u{1b}" && self.window().is_some_and(|w| w.global::<Pairing>().get_shown()) {
            self.close_pairing();
            return true;
        }
        let key = text.to_lowercase();
        let action = match (command, shift, key.as_str()) {
            (true, false, "z") => "undo",
            (true, true, "z") => "redo",
            (true, false, "g") => "toggle-mode",
            (true, true, "g") => "new-roll",
            (true, true, "y") => "screen-gamut",
            (true, false, "e") => "export",
            (true, false, "o") => "open",
            (true, false, "k") => "framing",
            (true, false, "-") => "smaller",
            (true, false, "=") | (true, false, "+") => "larger",
            (false, _, "]") => "bigger-brush",
            (false, _, "[") => "smaller-brush",
            (false, false, "0") => "medal-0",
            (false, false, "1") => "medal-1",
            (false, false, "2") => "medal-2",
            (false, false, "3") => "medal-3",
            _ => return false,
        };
        self.act(action);
        true
    }

    fn wire(me: &Rc<Ctl>, window: &MainWindow) {
        Ctl::wire_canvas(me, window);
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
                let double = slide.last_press.is_some_and(|t| now.duration_since(t) <= DOUBLE_CLICK);
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
                p.set(f, f.spec().base);
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
        window.global::<App>().on_key(move |text, control, meta, shift, option| {
            let command = if cfg!(target_os = "macos") { meta } else { control };
            p.key(&text, command, shift, option)
        });
        let p = me.clone();
        window.global::<Pairing>().on_close(move || p.act("close-pairing"));
        let p = me.clone();
        window.global::<PanelState>().on_auto_balance(move || p.act("auto-balance"));
        let p = me.clone();
        window.global::<PanelState>().on_pick(move |what| p.act(&what));

        let lib = window.global::<Library>();
        let p = me.clone();
        lib.on_row_clicked(move |i| p.row_clicked(i.max(0) as usize));
        let p = me.clone();
        lib.on_tile_clicked(move |i, toggle, extend| p.tile_clicked(i.max(0) as usize, toggle, extend));
        let p = me.clone();
        lib.on_tile_opened(move |i| p.tile_opened(i.max(0) as usize));
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
    /// `NegativeView`'s gestures: a drag pans, a double click toggles 100 %, the wheel zooms about
    /// the pointer, a touch surface pans unless ⌘ is held, a pinch zooms about its centre.
    fn wire_canvas(me: &Rc<Ctl>, window: &MainWindow) {
        let c = window.global::<Canvas>();
        let backing = move |p: &Rc<Ctl>| p.window().map_or(2.0, |w| f64::from(w.window().scale_factor()));

        let p = me.clone();
        c.on_resized(move || {
            let size = p.canvas_size();
            if let (Some(size), Some(full)) = (size, p.state.borrow().full_size) {
                p.state.borrow_mut().viewport.clamp(full, size);
            }
            p.request_render();
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
fn prepared(engine: &Shared, path: &str, primaries_converted: bool) -> Result<Arc<Negative>, String> {
    let decoded = render::decode(Path::new(path), primaries_converted)?;
    let negative = engine.lock().map_err(|_| "the render engine failed earlier")?.prepare(decoded)?;
    Ok(Arc::new(negative))
}
