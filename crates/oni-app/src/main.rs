//! `oni [folder]` opens a folder of scans, roll #8 by default; `--catalogue` or `ONI_CATALOGUE=1` shows the
//! design system alone; `ONI_BENCH=1` drags sixty Exposure notches, prints their centiles and quits.

use oni_app::app::{Options, Shell};
use oni_app::render::{Device, Engine};
use oni_app::{Catalogue, bench, timing};
use oni_core::{Archive, EditorStore};
use slint::ComponentHandle;
use slint::winit_030::{EventResult, WinitWindowAccessor, winit::event::WindowEvent};
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
    let gate = engine.clone();
    let folder = std::env::args_os().nth(1).map(PathBuf::from).unwrap_or_else(|| oni_fixtures::folder(oni_fixtures::RAW_DIR));
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let shell = Shell::new(Box::new(store), engine, Options { support, canvas, ..Options::default() })?;
    // Slint reconfigures the surface while it handles these two events: no other thread submits meanwhile.
    shell.window.window().on_winit_window_event(move |_, event| {
        if matches!(event, WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }) {
            gate.hold();
            let open = gate.clone();
            let _ = slint::invoke_from_event_loop(move || open.release());
        }
        EventResult::Propagate
    });
    shell.open_folder(&folder);
    if let Some(mode) = bench::requested() {
        bench::start(&shell.ctl, mode, |report| {
            timing::line(&report);
            let _ = slint::quit_event_loop();
        });
    }
    shell.window.run()?;
    timing::line(&shell.summary());
    Ok(())
}
