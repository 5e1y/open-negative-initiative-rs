//! `oni [folder or file]` opens a folder of scans, else the one last opened, else roll #8; `--catalogue` or `ONI_CATALOGUE=1` shows the
//! design system alone; `ONI_BENCH=1` drags sixty Exposure notches, prints their centiles and quits.

use oni_app::app::{Options, Shell};
use oni_app::render::{Device, Engine};
use oni_app::{Catalogue, bench, timing};
use oni_core::{Archive, EditorStore};
use slint::ComponentHandle;
use oni_app::viewport::{self, Wheel};
use slint::winit_030::{EventResult, WinitWindowAccessor, winit::event::MouseScrollDelta, winit::event::WindowEvent};
use slint::winit_030::winit::event::{ElementState, MouseButton};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalogue = std::env::args().any(|a| a == "--catalogue") || std::env::var_os("ONI_CATALOGUE").is_some_and(|v| v == "1");
    if catalogue {
        Catalogue::new()?.run()?;
        return Ok(());
    }
    let support = timing::support();
    let log = timing::log_path(&support);
    match timing::open_file(&log) {
        Ok(()) => timing::line("log: oni.log, in the app's support folder"),
        Err(e) => timing::line(&format!("no log file ({e}); stderr only")),
    }
    // One device for the pipeline, the display kernel and the window, so the canvas texture is shared.
    let gpu = Device::open()?;
    let info = gpu.adapter.get_info();
    timing::line(&format!("GPU: {} ({:?}, {:?}, driver {} {})", info.name, info.backend, info.device_type, info.driver, info.driver_info));
    let wgpu = slint::BackendSelector::new()
        .renderer_name("femtovg-wgpu".into())
        .require_wgpu_30(slint::wgpu_30::WGPUConfiguration::Manual {
            instance: gpu.instance.clone(),
            adapter: gpu.adapter.clone(),
            device: gpu.device.clone(),
            queue: gpu.queue.clone(),
        })
        .with_winit_custom_application_handler(Quitting)
        .with_winit_window_attributes_hook(oni_app::chrome::attributes)
        .select();
    let canvas = match wgpu {
        Ok(()) => {
            timing::line("Slint backend: femtovg-wgpu on the pipeline's device");
            Some((gpu.device.clone(), gpu.queue.clone()))
        }
        Err(e) => {
            timing::line(&format!("no wgpu renderer ({e}); the canvas is encoded on the CPU"));
            None
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
        if let Ok(mut e) = warming.lock() {
            e.prewarm();
        }
        timing::line(&format!("kernels compiled in {:.0} ms", started.elapsed().as_secs_f64() * 1000.0));
    });
    let gate = engine.clone();
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
    // winit names each file of one drop in turn: the drop is taken whole once every name is in.
    let (dropping, drops) = (shell.ctl.clone(), std::rc::Rc::new(std::cell::RefCell::new(Vec::<PathBuf>::new())));
    // Slint reconfigures the surface while it handles these two events: no other thread submits meanwhile.
    shell.window.window().on_winit_window_event(move |window, event| {
        if let WindowEvent::CursorMoved { position, .. } = event {
            cursor.set((position.x, position.y));
        }
        if let WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } = event {
            let (scale, (x, y)) = (f64::from(window.scale_factor()), cursor.get());
            oni_app::editors::pressed_anywhere(&pressed, (x / scale, y / scale));
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
                MouseScrollDelta::LineDelta(dx, dy) => Wheel { dx: f64::from(dx), dy: f64::from(dy), precise: false },
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
    match std::env::args_os().nth(1).map(PathBuf::from) {
        Some(path) => shell.add(&path),
        None if shell.open_remembered() => {}
        None => shell.open_folder(&oni_fixtures::folder(oni_fixtures::RAW_DIR)),
    }
    if let Some(mode) = bench::requested() {
        shell.open_first_when_read();
        bench::start(&shell.ctl, mode, |report| {
            timing::line(&report);
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
    slint::run_event_loop()?;
    // A quit that never asked the window to close still writes the open frame's last settings.
    if let Some(failure) = shell.last_writes() {
        timing::line(&format!("the open frame's settings could not be saved: {failure}"));
    }
    oni_app::patches::purge(&oni_app::patches::caches());
    timing::line(&shell.summary());
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

impl slint::winit_030::CustomApplicationHandler for Quitting {
    fn exiting(&mut self, _: &slint::winit_030::winit::event_loop::ActiveEventLoop) -> EventResult {
        if let Some(failure) = ON_EXIT.with(|e| e.borrow().as_ref().and_then(|write| write())) {
            timing::line(&format!("the open frame's settings could not be saved: {failure}"));
        }
        EventResult::Propagate
    }
}
