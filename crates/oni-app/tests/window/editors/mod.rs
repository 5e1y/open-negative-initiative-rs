//! The editors in the window, driven by pointer events: a point dragged on the curves plot and a
//! handle dragged on the levels track reach the store at the hand's pace, as one history step each.

pub mod curves;
pub mod panel;

use oni_app::app::{Options, Shell};
use oni_app::editors::{self, Plot};
use oni_app::{App, Bar, Editors, MainWindow, PanelState};
use oni_core::{Archive, EditorStore};
use oni_settings::spectrogram::{Band, Spectrogram};
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model, PhysicalSize};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

pub(super) fn press(rig: &common::Rig, at: (f64, f64)) {
    let position = LogicalPosition::new(at.0 as f32, at.1 as f32);
    rig.window.dispatch_event(WindowEvent::PointerMoved { position });
    rig.window.dispatch_event(WindowEvent::PointerPressed { position, button: PointerEventButton::Left });
    rig.pump();
}

pub(super) fn drag(rig: &common::Rig, from: (f64, f64), by: (f64, f64), steps: usize) {
    for k in 1..=steps {
        std::thread::sleep(Duration::from_millis(4));
        let t = k as f64 / steps as f64;
        let position = LogicalPosition::new((from.0 + by.0 * t) as f32, (from.1 + by.1 * t) as f32);
        rig.window.dispatch_event(WindowEvent::PointerMoved { position });
        rig.pump();
    }
}

pub(super) fn release(rig: &common::Rig, at: (f64, f64)) {
    let position = LogicalPosition::new(at.0 as f32, at.1 as f32);
    rig.window.dispatch_event(WindowEvent::PointerReleased { position, button: PointerEventButton::Left });
    rig.pump();
}

/// A little-endian LinearRaw TIFF of `w` by `h`, 8-bit RGB in one strip, as a scanner writes it.
pub(super) fn linear_raw_8_bit(w: u32, h: u32) -> Vec<u8> {
    let data = vec![128u8; (w * h * 3) as usize];
    let entries: [(u16, u16, u32); 9] = [
        (256, 4, w),
        (257, 4, h),
        (258, 3, 8),
        (259, 3, 1),
        (262, 3, 34892),
        (273, 4, 8),
        (277, 3, 3),
        (278, 4, h),
        (279, 4, data.len() as u32),
    ];
    let mut out = b"II".to_vec();
    out.extend(42u16.to_le_bytes());
    out.extend((8 + data.len() as u32).to_le_bytes());
    out.extend(&data);
    out.extend((entries.len() as u16).to_le_bytes());
    for (tag, kind, value) in entries {
        out.extend(tag.to_le_bytes());
        out.extend(kind.to_le_bytes());
        out.extend(1u32.to_le_bytes());
        if kind == 3 {
            out.extend((value as u16).to_le_bytes());
            out.extend(0u16.to_le_bytes());
        } else {
            out.extend(value.to_le_bytes());
        }
    }
    out.extend(0u32.to_le_bytes());
    out
}

/// The editor on one RAW frame, the panel on `tab` laid out by a first drawn frame.
pub(super) struct Opened {
    pub(super) rig: common::Rig,
    pub(super) shell: Shell,
    pub(super) gpu: oni_app::render::Device,
    pub(super) dir: PathBuf,
    pub(super) frame: String,
}

pub(super) fn opened(label: &str) -> Option<Opened> {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => {
            eprintln!("{e}: the window is not exercised");
            return None;
        }
    };
    let shots = std::env::var_os("ONI_SHOTS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let dir = std::env::temp_dir().join(format!("oni-app-editors-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let frame = dir.join("frame.NEF").to_string_lossy().into_owned();
    let mut store = EditorStore::new(Archive::new(vec![frame.clone()], |_: &Path| true));
    store.open(&frame).unwrap();
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support: dir.join("support"), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.set_size(PhysicalSize::new(1280, 2200));
    shell.window.show().unwrap();
    // The app launches on the gallery; the store's open frame is shown in the editor.
    shell.window.global::<App>().set_gallery(false);
    Some(Opened { rig, shell, gpu, dir, frame })
}

/// Lays out the panel on `tab`, as the app's next frame does.
pub(super) fn on_tab(o: &Opened, tab: i32, unfold: impl FnOnce(&Editors)) {
    o.shell.window.global::<PanelState>().set_tab(tab);
    unfold(&o.shell.window.global::<Editors>());
    o.rig.shot("editors-opened");
    o.rig.settle();
}
