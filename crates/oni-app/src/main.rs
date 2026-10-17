//! `oni [folder]`: opens a folder of scans, roll #8 when none is named. `ONI_BENCH=1` drags sixty
//! notches on Exposure once the first frame is shown, prints the notch centiles and quits.

use oni_app::app::{Options, Shell};
use oni_app::render::{Device, Engine};
use oni_app::{bench, timing};
use oni_core::{Archive, EditorStore};
use slint::ComponentHandle;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    match wgpu {
        Ok(()) => timing::line("Slint backend: femtovg-wgpu on the pipeline's device"),
        Err(e) => timing::line(&format!("no wgpu renderer ({e}); the canvas is encoded on the CPU")),
    }
    let engine = Engine::shared(&gpu);
    let folder = std::env::args_os().nth(1).map(PathBuf::from).unwrap_or_else(|| oni_fixtures::folder(oni_fixtures::RAW_DIR));
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let shell = Shell::new(Box::new(store), engine, Options::default())?;
    shell.open_folder(&folder);
    if bench::requested() {
        bench::start(&shell.ctl, |report| {
            timing::line(&report);
            let _ = slint::quit_event_loop();
        });
    }
    shell.window.run()?;
    timing::line(&shell.summary());
    Ok(())
}
