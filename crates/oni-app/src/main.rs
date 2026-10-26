//! `oni [folder or file]` opens a folder of scans, else the one last opened, else nothing;
//! `ONI_BENCH=1` drags sixty Exposure notches, prints their centiles and quits.
// A Windows release opens its window alone, with no console beside it.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use oni_app::app::{Options, Shell};
use oni_app::render::{Device, Engine};
use oni_app::{bench, timing};
use oni_core::{Archive, EditorStore};
use slint::ComponentHandle;
use oni_app::viewport::{self, Wheel};
use slint::winit_030::{EventResult, WinitWindowAccessor, winit::event::MouseScrollDelta, winit::event::WindowEvent};
use slint::winit_030::winit::event::{ElementState, MouseButton};
use std::path::PathBuf;

#[cfg(target_os = "macos")]
#[global_allocator]
static ALLOCATOR: oni_app::mapped::Mapped = oni_app::mapped::Mapped;

/// A Mac app starts with 256 descriptors: a roll of RAWs decoded together, its sidecars and the index
/// pass that, so the soft limit is raised toward the hard one.
#[cfg(target_os = "macos")]
fn more_descriptors() {
    // SAFETY: plain getrlimit/setrlimit on this process's own limit.
    unsafe {
        let mut limit = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) == 0 && limit.rlim_cur < 10_240 {
            limit.rlim_cur = limit.rlim_max.min(10_240);
            libc::setrlimit(libc::RLIMIT_NOFILE, &limit);
        }
    }
}

/// A start that fails is written to the log and said in a window of the system's: a windowed build
/// and an app opened from the Finder have no stderr.
fn main() {
    if let Err(e) = run() {
        timing::fatal(&e.to_string());
        oni_app::instance::could_not_start(&timing::support(), &e.to_string());
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    more_descriptors();
    let support = timing::support();
    // Before any write, the log included: a second launch on the same folder leaves it as it is.
    let _instance = match oni_app::instance::hold(&support) {
        oni_app::instance::Held::Mine(instance) => Some(instance),
        oni_app::instance::Held::Elsewhere => {
            oni_app::instance::refused(&support);
            std::process::exit(2);
        }
        oni_app::instance::Held::Unknown(e) => {
            timing::line(&format!("no instance lock in the support folder ({e})"));
            None
        }
    };
    let log = timing::log_path(&support);
    match timing::open_file(&log) {
        Ok(()) => timing::line("log: oni.log, in the app's support folder"),
        Err(e) => timing::line(&format!("no log file ({e}); stderr only")),
    }
    timing::keep_panics();
    // One device for the pipeline, the display kernel and the window, so the canvas texture is shared.
    let gpu = Device::open()?;
    let info = gpu.adapter.get_info();
    timing::line(&format!("GPU: {} ({:?}, {:?}, driver {} {})", info.name, info.backend, info.device_type, info.driver, info.driver_info));
    let selector = slint::BackendSelector::new()
        .renderer_name("femtovg-wgpu".into())
        .require_wgpu_30(slint::wgpu_30::WGPUConfiguration::Manual {
            instance: gpu.instance.clone(),
            adapter: gpu.adapter.clone(),
            device: gpu.device.clone(),
            queue: gpu.queue.clone(),
        })
        .with_winit_custom_application_handler(Quitting)
        .with_winit_window_attributes_hook(oni_app::chrome::attributes);
    let wgpu = behind(selector).select();
    let canvas = match wgpu {
        Ok(()) => {
            timing::line("Slint backend: femtovg-wgpu on the pipeline's device");
            true
        }
        Err(e) => {
            timing::line(&format!("no wgpu renderer ({e}); the canvas is read back from the GPU"));
            false
        }
    };
    let engine = Engine::shared(&gpu);
    {
        let mut e = engine.lock()?;
        e.support = Some(support.clone());
        e.caches = Some(oni_app::patches::caches());
    }
    let warming = engine.clone();
    std::thread::spawn(move || {
        let started = std::time::Instant::now();
        warming.prewarm();
        timing::line(&format!("kernels compiled in {:.0} ms", started.elapsed().as_secs_f64() * 1000.0));
    });
    let gate = engine.clone();
    let meter = engine.clone();
    // Whole decodes kept for the next opening, about 145 MB a 24 Mpx frame.
    oni_decode::cache::keep_in(oni_app::system::caches().join("Decodes"), DECODES_KEPT);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    // The earlier version ran on macOS alone: there its RAW settings are corrected once, never under a bench.
    #[cfg(target_os = "macos")]
    let transition: Option<std::sync::Arc<dyn oni_transition::AppleDecode>> =
        bench::requested().is_none().then(|| std::sync::Arc::new(oni_transition::apple::CoreImage) as _);
    #[cfg(not(target_os = "macos"))]
    let transition = None;
    // The window's adapter is made here, while the hook still knows the main window is the one built.
    let shell = oni_app::chrome::unified(|| Shell::new(Box::new(store), engine, Options { support, caches: oni_app::system::caches(), canvas, require_roll: true, pasteboard: oni_app::pasteboard::general(), computer: oni_library::navigator::Computer::system(), updates: bench::requested().is_none().then(oni_app::app::Updater::system), reporter: Options::online_reporter(), transition, ..Options::default() }).inspect(|s| {
        s.window.window();
    }))?;
    // `Diagnostics.startWatchdog`: the interface thread beats, a thread of its own watches it.
    let beat = slint::Timer::default();
    let watched = shell.ctl.clone();
    beat.start(slint::TimerMode::Repeated, oni_app::watchdog::BEAT, move || {
        let witness = watched.state.borrow().witness();
        oni_app::watchdog::beat(std::time::Instant::now(), witness);
    });
    oni_app::watchdog::start();
    let room = shell.window.as_weak();
    shell.window.global::<oni_app::Bar>().on_room_pressed(move || {
        if let Some(w) = room.upgrade() {
            oni_app::chrome::room_pressed(w.window());
        }
    });
    let quitting = shell.ctl.clone();
    ON_EXIT.with(|e| *e.borrow_mut() = Some(Box::new(move || quitting.last_writes())));
    let (pressed, cursor) = (shell.ctl.clone(), std::cell::Cell::new((0.0, 0.0)));
    let lit = shell.window.as_weak();
    let focus = shell.ctl.clone();
    let told = shell.window.as_weak();
    // winit names each file of one drop in turn: the drop is taken whole once every name is in.
    let (dropping, drops) = (shell.ctl.clone(), std::rc::Rc::new(std::cell::RefCell::new(Vec::<PathBuf>::new())));
    // Slint reconfigures the surface while it handles these two events: no other thread submits meanwhile.
    shell.window.window().on_winit_window_event(move |window, event| {
        if let WindowEvent::CursorMoved { position, .. } = event {
            cursor.set((position.x, position.y));
        }
        if let WindowEvent::MouseInput { state: ElementState::Pressed, button, .. } = event {
            if let Some(w) = told.upgrade() {
                oni_app::outside::pressed(&w);
            }
            if *button == MouseButton::Left {
                let (scale, (x, y)) = (f64::from(window.scale_factor()), cursor.get());
                oni_app::editors::pressed_anywhere(&pressed, (x / scale, y / scale));
            }
        }
        if let WindowEvent::Focused(focused) = event {
            focus.focus_changed(*focused);
        }
        if let WindowEvent::DroppedFile(path) = event {
            let first = drops.borrow().is_empty();
            drops.borrow_mut().push(path.clone());
            if first {
                let (drops, dropping) = (drops.clone(), dropping.clone());
                slint::Timer::single_shot(std::time::Duration::ZERO, move || {
                    let paths = std::mem::take(&mut *drops.borrow_mut());
                    dropping.dropped(&paths);
                });
            }
        }
        if let WindowEvent::MouseWheel { delta, .. } = event {
            let wheel = match *delta {
                MouseScrollDelta::LineDelta(dx, dy) => viewport::lines(dx, dy, cfg!(windows)),
                MouseScrollDelta::PixelDelta(p) => {
                    let scale = f64::from(window.scale_factor());
                    Wheel { dx: p.x / scale, dy: p.y / scale, precise: true }
                }
            };
            viewport::note_wheel(wheel);
        }
        if matches!(event, WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }) {
            if let Some(w) = lit.upgrade() {
                lights(&w);
            }
            gate.hold();
            let open = gate.clone();
            let _ = slint::invoke_from_event_loop(move || open.release());
        }
        EventResult::Propagate
    });
    // A path named opens it or nothing; the reference roll opens only where a check or a bench names it.
    match std::env::args_os().nth(1).map(PathBuf::from) {
        Some(path) => shell.add(&path),
        None if shell.open_remembered() => {}
        None if std::env::var_os(oni_fixtures::RAW_DIR.0).is_some() || bench::requested().is_some() => {
            shell.open_folder(&oni_fixtures::folder(oni_fixtures::RAW_DIR))
        }
        None => {}
    }
    if let Some(mode) = bench::requested() {
        shell.open_first_when_read();
        bench::start(&shell.ctl, mode, move |report| {
            timing::line(&report);
            if let Ok(e) = meter.lock() {
                let s = e.stats();
                timing::line(&format!("graph textures: peak {} MB, {} MB at the end", s.peak_bytes >> 20, s.live_bytes >> 20));
            }
            let _ = slint::quit_event_loop();
        });
    }
    // Probes and benches drive the window at once, as Swift skips its splash under a probe.
    if bench::requested().is_none() {
        shell.welcome_if_due();
        shell.offer_wrapped();
        let main = shell.window.as_weak();
        oni_app::splash::show(move || {
            if let Some(w) = main.upgrade() {
                let _ = w.show();
                lights(&w);
            }
        })?;
    } else {
        shell.window.show()?;
        lights(&shell.window);
    }
    let _hz = if bench::requested().is_none() { oni_app::hz::watch(&shell.window) } else { None };
    slint::run_event_loop()?;
    // A quit that never asked the window to close still writes the open frame's last settings.
    if let Some(failure) = shell.last_writes() {
        timing::line(&format!("the open frame's settings could not be saved: {failure}"));
    }
    oni_app::patches::purge(&oni_app::patches::caches());
    timing::line(&shell.summary());
    timing::flush();
    Ok(())
}

/// The bar makes room for the traffic lights wherever they show: none in full screen.
fn lights(w: &oni_app::MainWindow) {
    let bar = w.global::<oni_app::Bar>();
    bar.set_traffic_lights(oni_app::chrome::traffic_lights(w.window(), bar.get_height()));
}

thread_local! {
    /// What a quit writes, set once the shell stands; ⌘Q ends AppKit's loop without closing the window.
    static ON_EXIT: std::cell::RefCell<Option<Box<dyn Fn() -> Option<String>>>> = const { std::cell::RefCell::new(None) };
}

/// The application's exit, seen before AppKit ends the process, so ⌘Q loses no settings.
struct Quitting;

/// A bench, or `ONI_BACKGROUND=1`, opens behind the other apps' windows and leaves the focus where it is,
/// so the hand keeps working while a check drives the window.
fn behind(selector: slint::BackendSelector) -> slint::BackendSelector {
    let background = bench::requested().is_some() || std::env::var_os("ONI_BACKGROUND").is_some_and(|v| v == "1");
    #[cfg(target_os = "macos")]
    if background {
        use slint::winit_030::winit::platform::macos::EventLoopBuilderExtMacOS;
        let mut builder = slint::winit_030::winit::event_loop::EventLoop::<slint::winit_030::SlintEvent>::with_user_event();
        builder.with_activate_ignoring_other_apps(false);
        return selector
            .with_winit_event_loop_builder(builder)
            .with_winit_window_attributes_hook(|a| oni_app::chrome::attributes(a).with_active(false));
    }
    let _ = background;
    selector
}

impl slint::winit_030::CustomApplicationHandler for Quitting {
    fn exiting(&mut self, _: &slint::winit_030::winit::event_loop::ActiveEventLoop) -> EventResult {
        if let Some(failure) = ON_EXIT.with(|e| e.borrow().as_ref().and_then(|write| write())) {
            timing::line(&format!("the open frame's settings could not be saved: {failure}"));
        }
        timing::flush();
        EventResult::Propagate
    }
}

/// The disk the kept decodes may take, a roll's worth.
const DECODES_KEPT: u64 = 4 << 30;
