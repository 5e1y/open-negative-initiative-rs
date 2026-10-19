//! The top bar as the window widens: its keys keep their widths and only the room between them grows.

use oni_app::app::{Options, Shell};
use oni_core::{Archive, EditorStore};
use slint::platform::software_renderer::Rgb565Pixel;
use slint::platform::WindowEvent;
use slint::ComponentHandle;
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

/// The window drawn at `w` by 800, as rows of RGB565 pixels.
fn drawn(rig: &common::Rig, w: u32) -> (usize, Vec<u16>) {
    rig.window.set_size(slint::PhysicalSize::new(w, 800));
    rig.settle();
    rig.window.request_redraw();
    let (w, h) = (w as usize, 800);
    let mut buffer = vec![Rgb565Pixel(0); w * h];
    rig.window.draw_if_needed(|r| {
        r.render(&mut buffer, w);
    });
    (w, buffer.into_iter().map(|p| p.0).collect())
}

/// Along the row just under the top edge of the bar's last key, from the window's right edge
/// leftwards, the runs of key and of ground: the last key, the gap, the key before, the room before it.
fn runs(w: usize, px: &[u16]) -> Vec<usize> {
    let at = |x: usize, y: usize| px[y * w + x];
    let x = w - 30;
    let ground = at(x, 44);
    let top = (44..120).find(|&y| at(x, y) != ground).expect("a key at the bar's end") + 2;
    let key = |i: usize| at(i, top) != ground;
    let mut out = Vec::new();
    let mut i = (x..w).find(|&i| !key(i)).unwrap_or(w);
    while out.len() < 4 && i > 0 {
        let kind = key(i - 1);
        let start = (0..i).rev().find(|&j| key(j) != kind).map_or(0, |j| j + 1);
        out.push(i - start);
        i = start;
    }
    out
}

pub(crate) fn the_bars_keys_keep_their_widths_as_the_window_widens() {
    let root = std::env::temp_dir().join(format!("oni-app-toolbar-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (support, caches) = (root.join("Support"), root.join("Caches"));
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the bar is not exercised"),
    };
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    shell.window.show().unwrap();
    rig.until("the bar laid", Duration::from_secs(5), || true);
    // ADD FOLDER, the gap, FEEDBACKS, then the room before FEEDBACKS.
    let keys = |w: u32| {
        let (w, px) = drawn(&rig, w);
        runs(w, &px)
    };
    let (narrow, wide) = (keys(1280), keys(1700));
    println!("shot {}", rig.shot("toolbar-1700").display());
    assert_eq!(narrow[..3], wide[..3], "the keys and their gap keep their widths: {narrow:?} {wide:?}");
    // Twin: the room before them does grow with the window.
    assert!(wide[3] > narrow[3] + 100, "the room before the keys grows: {narrow:?} {wide:?}");
}
