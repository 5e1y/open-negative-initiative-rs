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
        Ok(()) => timing::line(&format!("log: {}", log.display())),
        Err(e) => timing::line(&format!("no log file at {} ({e}); stderr only", log.display())),
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
    let gate = engine.clone();
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let shell = Shell::new(Box::new(store), engine, Options { support, canvas, ..Options::default() })?;
    let (pressed, cursor) = (shell.ctl.clone(), std::cell::Cell::new((0.0, 0.0)));
    // Slint reconfigures the surface while it handles these two events: no other thread submits meanwhile.
    shell.window.window().on_winit_window_event(move |window, event| {
        if let WindowEvent::CursorMoved { position, .. } = event {
            cursor.set((position.x, position.y));
        }
        if let WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } = event {
            let (scale, (x, y)) = (f64::from(window.scale_factor()), cursor.get());
            oni_app::editors::pressed_anywhere(&pressed, (x / scale, y / scale));
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
        let main = shell.window.as_weak();
        oni_app::splash::show(move || {
            if let Some(w) = main.upgrade() {
                let _ = w.show();
            }
        })?;
    } else {
        shell.window.show()?;
    }
    slint::run_event_loop()?;
    oni_app::patches::purge(&oni_app::patches::caches());
    timing::line(&shell.summary());
    Ok(())
}
