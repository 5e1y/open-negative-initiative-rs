//! `oni [folder]`: opens a folder of scans, roll #8 when none is named.

use oni_app::app::{Options, Shell};
use oni_core::{Archive, EditorStore};
use slint::ComponentHandle;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The wgpu renderer, so the canvas goes through the display kernel; the software one otherwise.
    let wgpu = slint::BackendSelector::new()
        .renderer_name("femtovg-wgpu".into())
        .require_wgpu_30(slint::wgpu_30::WGPUConfiguration::default())
        .select();
    if let Err(e) = wgpu {
        eprintln!("no wgpu renderer ({e}); the canvas is encoded on the CPU");
    }
    let folder = std::env::args_os().nth(1).map(PathBuf::from).unwrap_or_else(oni_ui_poc::frame::roll_dir);
    // Every frame reads as a RAW mosaic: the shell decodes through LibRaw only, and sorting on the
    // bytes arrives with the decode router.
    let store = EditorStore::new(Archive::new(Vec::new(), |_: &std::path::Path| true));
    let shell = Shell::new(Box::new(store), Box::new(oni_app::render::Stage47), Options::default())?;
    shell.open_folder(&folder);
    shell.window.run()?;
    Ok(())
}
