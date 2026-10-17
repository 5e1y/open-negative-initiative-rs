//! `oni [folder]`: opens a folder of scans, roll #8 when none is named.

use oni_app::app::{Options, Shell};
use oni_app::render::{Device, Engine};
use oni_core::{Archive, EditorStore};
use slint::ComponentHandle;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // One device for the pipeline, the display kernel and the window, so the canvas texture is shared.
    let gpu = Device::open()?;
    let wgpu = slint::BackendSelector::new()
        .renderer_name("femtovg-wgpu".into())
        .require_wgpu_30(slint::wgpu_30::WGPUConfiguration::Manual {
            instance: gpu.instance.clone(),
            adapter: gpu.adapter.clone(),
            device: gpu.device.clone(),
            queue: gpu.queue.clone(),
        })
        .select();
    if let Err(e) = wgpu {
        eprintln!("no wgpu renderer ({e}); the canvas is encoded on the CPU");
    }
    let engine = Engine::shared(&gpu);
    let folder = std::env::args_os().nth(1).map(PathBuf::from).unwrap_or_else(|| oni_fixtures::folder(oni_fixtures::RAW_DIR));
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let shell = Shell::new(Box::new(store), engine, Options::default())?;
    shell.open_folder(&folder);
    shell.window.run()?;
    Ok(())
}
