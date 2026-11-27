//! `oni [folder or file]` opens a folder of scans, else the one last opened, else nothing;
//! `ONI_BENCH=1` drags sixty Exposure notches, prints their centiles and quits.
// A Windows release opens its window alone, with no console beside it.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use oni_app::app::{Options, Shell};
use oni_app::render::{Device, Engine, Kernels, Shared};
use oni_app::{MainWindow, bench, starter, timing};
use oni_core::{Archive, EditorStore};
use oni_i18n::marked;
use slint::ComponentHandle;
use oni_app::viewport::{self, Wheel};
use slint::winit_030::{EventResult, WinitWindowAccessor, winit::event::MouseScrollDelta, winit::event::WindowEvent};
use slint::winit_030::winit::event::{ElementState, MouseButton};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

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
        oni_app::instance::could_not_start(&oni_app::system::support(), &e.to_string());
        std::process::exit(1);
    }
}

/// The shell once its step of the launch has built it.
type Standing = Rc<RefCell<Option<Shell>>>;

#[path = "main/watch.rs"]
mod watch;
use watch::watch;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    more_descriptors();
    let began = Instant::now();
    let support = oni_app::system::support();
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
    // Before anything reads a support file: those whose shape changed are brought up to date once.
    let brought = oni_app::migration::support(&support);
    // Slint's MCP server, an evaluation's driver, sends keys no hand holds: the system cannot confirm them.
    if std::env::var_os("SLINT_MCP_PORT").is_none() {
        oni_app::system::verify_held_keys();
    }
    timing::line(&format!("system: {}, ONI {}", oni_app::system::described(), env!("CARGO_PKG_VERSION")));
    // One device for the pipeline, the display kernel and the window, so the canvas texture is shared.
    let gpu = Device::open()?;
    let graphics = began.elapsed();
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
    let interface = began.elapsed() - graphics;
    let engine = Engine::shared(&gpu);
    {
        let mut e = engine.lock()?;
        e.support = Some(support.clone());
        e.caches = Some(oni_app::patches::caches());
    }
    let kernels = std::sync::Arc::new(Kernels::default());
    // wgpu refuses a surface configured while another thread submits (`GpuWaitTimeout`), which leaves the window no
    // surface: the prewarm's submissions wait at the door until winit has made the window and configured its first.
    engine.hold();
    let (warming, counted) = (engine.clone(), kernels.clone());
    std::thread::spawn(move || {
        let started = Instant::now();
        warming.prewarm_counted(&counted);
        timing::line(&format!("kernels compiled in {:.0} ms", started.elapsed().as_secs_f64() * 1000.0));
    });
    // The window's adapter is made here, while the hook still knows the main window is the one built.
    let building = Instant::now();
    let window = oni_app::chrome::unified(|| {
        MainWindow::new().inspect(|w| {
            w.window();
        })
    })?;
    let built = building.elapsed();
    let shell: Standing = Rc::default();
    let failed: Rc<RefCell<Option<slint::PlatformError>>> = Rc::default();
    let hz: Rc<RefCell<Option<slint::Timer>>> = Rc::default();
    let _watching = watch(&window, &shell, &engine);
    let steps = vec![
        starter::step(marked("Reading the library"), {
            let support = support.clone();
            move || {
                // Whole decodes kept for the next opening, about 145 MB a 24 Mpx frame.
                oni_decode::cache::keep_in(oni_app::system::caches().join("Decodes"), DECODES_KEPT);
                // A sidecar of an earlier format asks its original for the camera matrix and the scan size.
                oni_app::migration::facts::install(&support);
                if !oni_app::wrapped::alone() {
                    oni_app::migration::facts::sweep(&support, &oni_library::index::LibraryIndex::default_path(&oni_app::system::caches()));
                }
            }
        }),
        starter::step(marked("Building the app"), {
            let (built, shell, failed, engine) = (window.clone_strong(), shell.clone(), failed.clone(), engine.clone());
            move || {
                let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
                // The folded bench pairs a phone, so it offers the link whatever the switch says.
                // The year alone offers no phone, asks no site and looks for no update: it only reads.
                let (alone, benched) = (oni_app::wrapped::alone(), bench::requested());
                let phone = !alone && (oni_app::phone::offered() || benched == Some(bench::Mode::Folded));
                // A bench copies to a board of its own: the person's clipboard is theirs.
                let pasteboard: Box<dyn oni_app::pasteboard::Board> = if benched.is_some() { Box::new(oni_app::pasteboard::Memory::default()) } else { oni_app::pasteboard::general() };
                // A bench never asks The Lens DB, so its timings never wait on the network.
                let tldb = if alone || benched.is_some() { oni_app::app::TldbSite::None } else { oni_app::app::TldbSite::launch(&support) };
                let options = Options { support, tldb, caches: oni_app::system::caches(), screen: if canvas { oni_app::display::WINDOW } else { oni_app::display::Screen::Srgb }, canvas, require_roll: true, pasteboard, computer: oni_library::navigator::Computer::system(), updates: (!alone && benched.is_none()).then(oni_app::app::Updater::system), reporter: Options::online_reporter(), phone, brought, ..Options::default() };
                match Shell::standing(built, store, engine, options) {
                    Ok(standing) => {
                        let quitting = standing.ctl.clone();
                        ON_EXIT.with(|e| *e.borrow_mut() = Some(Box::new(move || quitting.last_writes())));
                        starter::more(standing.rising());
                        *shell.borrow_mut() = Some(standing);
                    }
                    Err(e) => {
                        *failed.borrow_mut() = Some(e);
                        let _ = slint::quit_event_loop();
                    }
                }
            }
        }),
        starter::step(marked("Opening the last folder"), {
            let shell = shell.clone();
            move || {
                let standing = shell.borrow();
                let Some(shell) = standing.as_ref() else { return };
                // A path named opens it or nothing; the reference roll opens only where a check or a bench names it.
                match std::env::args_os().nth(1).map(PathBuf::from) {
                    _ if oni_app::wrapped::alone() => {}
                    Some(path) => shell.add(&path),
                    None if shell.open_remembered() => {}
                    None if std::env::var_os(oni_fixtures::RAW_DIR.0).is_some() || bench::requested().is_some() => {
                        shell.open_folder(&oni_fixtures::folder(oni_fixtures::RAW_DIR))
                    }
                    None => {}
                }
                if bench::requested().is_some() {
                    shell.open_first_when_read();
                }
            }
        }),
        starter::step(marked("Preparing the photo opened last"), {
            let shell = shell.clone();
            move || {
                if let Some(shell) = shell.borrow().as_ref().filter(|_| bench::requested().is_none() && !oni_app::wrapped::alone()) {
                    shell.prepare_ahead();
                }
            }
        }),
    ];
    let ready = {
        let (shell, hz, meter) = (shell.clone(), hz.clone(), engine.clone());
        move || {
            let standing = shell.borrow();
            let Some(shell) = standing.as_ref() else { return };
            if let Some(mode) = bench::requested() {
                bench::start(&shell.ctl, mode, move |report| {
                    timing::line(&report);
                    if let Ok(e) = meter.lock() {
                        let s = e.stats();
                        timing::line(&format!("graph textures: peak {} MB, {} MB at the end", s.peak_bytes >> 20, s.live_bytes >> 20));
                    }
                    let _ = slint::quit_event_loop();
                });
            } else if oni_app::wrapped::alone() {
                shell.ctl.act(oni_app::Act::Wrapped);
            } else {
                shell.launch_sheets();
                shell.migrate(&oni_app::system::caches());
                shell.offer_wrapped();
                *hz.borrow_mut() = oni_app::hz::watch(&shell.window);
            }
            lights(&shell.window);
        }
    };
    let before = vec![(marked("Starting the graphics card"), graphics), (marked("Starting the interface"), interface), (marked("Building the window"), built)];
    starter::launch(&window, began, before, steps, kernels, ready)?;
    lights(&window);
    let (made, door) = (window.as_weak(), engine.clone());
    let _ = slint::spawn_local(async move {
        if let Some(w) = made.upgrade() {
            let _ = w.window().winit_window().await;
        }
        door.release();
    });
    // winit makes the window once the loop runs: its full-screen transitions are followed from then.
    let after = window.as_weak();
    let _ = slint::spawn_local(async move {
        let Some(w) = after.upgrade() else { return };
        let Ok(made) = w.window().winit_window().await else { return };
        oni_app::chrome::after_full_screen(&made, move |entered| {
            if let Some(w) = after.upgrade() {
                lights(&w);
                let room = w.global::<oni_app::Bar>().get_traffic_lights();
                timing::line(&format!("full screen {}: the bar keeps {room:.0} pt for the lights", if entered { "entered" } else { "left" }));
            }
        });
    });
    oni_app::on_screen::shown(window.as_weak());
    slint::run_event_loop()?;
    if let Some(e) = failed.borrow_mut().take() {
        return Err(e.into());
    }
    let standing = shell.borrow_mut().take();
    if let Some(shell) = standing {
        // A quit that never asked the window to close still writes the open frame's last settings.
        if let Some(failure) = shell.last_writes() {
            timing::line(&format!("the open frame's settings could not be saved: {failure}"));
        }
        oni_app::patches::purge(&oni_app::patches::caches());
        timing::line(&shell.summary());
    }
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
