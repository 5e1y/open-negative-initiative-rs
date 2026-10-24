//! A photo whose original cannot be read says why on the canvas, before any decode, over its cached
//! thumbnail and read-only; edits a disk gone refused are kept and asked about. The disk is a folder
//! under a volumes folder of the check's own, never `/Volumes`.

use oni_app::app::{self, Options, Shell};
use oni_app::fields::Field;
use oni_app::{Absent, App, Canvas, Library, Store as StoreGlobal, Unsaved};
use oni_core::{Archive, EditorStore};
use oni_library::navigator::Computer;
use slint::ComponentHandle;
use slint::platform::WindowEvent;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

struct Bench {
    rig: common::Rig,
    shell: Shell,
    root: PathBuf,
    roll: PathBuf,
}

/// A roll on the volume `T7`, its thumbnails drawn, opened on the gallery.
fn bench(label: &str, extra: impl Fn(&Path)) -> Option<Bench> {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => {
            eprintln!("{e}: a missing original is not exercised");
            return None;
        }
    };
    let root = std::env::temp_dir().join(format!("oni-app-missing-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    // The spelling the app keeps: Windows' temp folder is a short `RUNNER~1` name and uses `\`.
    let root = PathBuf::from(oni_library::paths::canonical(&root));
    let roll = root.join("Volumes/T7/Scans/Roll 12");
    for (name, tone) in [("A00.tif", 0.3), ("B00.tif", 0.5), ("C00.tif", 0.7)] {
        common::plain_tiff(&roll.join(name), 48, 32, tone);
    }
    extra(&roll);
    std::fs::create_dir_all(root.join("home")).unwrap();
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let computer = Computer { home: root.join("home").to_string_lossy().into_owned(), volumes: root.join("Volumes").to_string_lossy().into_owned() };
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), computer, ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    shell.open_folder(&root.join("Volumes/T7/Scans"));
    let ctl = shell.ctl.clone();
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    rig.until("the thumbnails", Duration::from_secs(120), || ctl.state.borrow().loaded() >= 3);
    rig.settle();
    Some(Bench { rig, shell, root, roll })
}

impl Bench {
    /// Opens the tile captioned `caption` from the gallery.
    fn open(&self, caption: &str) {
        let w = &self.shell.window;
        if !w.global::<App>().get_gallery() {
            self.shell.ctl.act(oni_app::Act::ToggleMode);
            self.rig.settle();
        }
        let tile = app::tiles(w).into_iter().find(|t| t.caption == caption).unwrap_or_else(|| panic!("{caption} is shown"));
        w.global::<Library>().invoke_tile_opened(tile.index);
        self.rig.pump();
    }

    fn said(&self) -> (String, String) {
        let a = self.shell.window.global::<Absent>();
        (a.get_title().to_string(), a.get_detail().to_string())
    }
}

pub(crate) fn a_photo_whose_original_is_gone_says_so_on_the_canvas() {
    let Some(b) = bench("gone", |roll| {
        std::fs::write(roll.join("Bad.tif"), b"II*\0not a picture").unwrap();
        std::fs::write(roll.join("E00.tif"), b"").unwrap();
    }) else {
        return;
    };
    let w = &b.shell.window;
    let (canvas, store) = (w.global::<Canvas>(), w.global::<StoreGlobal>());

    // Twin: a file really broken is the decoder's to refuse, in its own words.
    b.open("Bad");
    b.rig.until("Bad refused", Duration::from_secs(60), || !canvas.get_failure().is_empty());
    assert_eq!(canvas.get_failure(), "Bad.tif: could not be decoded");
    assert_eq!(b.said().0, "", "a file there and of a format ONI reads is left to its decode");

    // A copy interrupted at 0 bytes is said so, never decoded.
    b.open("E00");
    b.rig.settle();
    assert_eq!(b.said().0, "“E00.tif” is empty (0 bytes)");
    assert_eq!(canvas.get_failure(), "", "the decoder's word is not laid over it");
    assert!(!canvas.get_busy() && !canvas.get_status().contains("decoding"));

    // Moved away while its disk stays: found missing before any decode, the thumbnail kept, read-only.
    std::fs::rename(b.roll.join("A00.tif"), b.root.join("A00.tif")).unwrap();
    b.open("A00");
    b.rig.settle();
    let (title, detail) = b.said();
    assert_eq!(title, "“A00.tif” cannot be found");
    assert!(detail.contains("moved or deleted"), "{detail}");
    assert_eq!(canvas.get_status(), title, "no later status of the opening covers it");
    assert!(canvas.get_frame().size().width > 0, "the cached thumbnail stays on the canvas");
    assert!(canvas.get_read_only() && canvas.get_failure() == "");
    assert!(store.invoke_press(Field::Exposure.index(), false), "a slider's press is refused, so no drag starts");
    store.invoke_release(Field::Exposure.index());
    assert_eq!(w.global::<Absent>().get_folder(), oni_library::paths::normal(&b.roll.to_string_lossy()).as_ref(), "Show the Folder opens its own");
    b.rig.shot("missing-gone");
    w.global::<Absent>().invoke_back();
    b.rig.settle();
    assert!(w.global::<App>().get_gallery(), "Back to the Gallery goes back");

    // Edited, then its disk unplugged: leaving the photo keeps the edits and asks.
    b.open("B00");
    b.rig.until("B00 rendered", Duration::from_secs(120), || b.shell.ctl.state.borrow().renders() >= 1);
    assert_eq!(b.said().0, "", "twin: a photo there says nothing");
    assert!(!store.invoke_press(Field::Exposure.index(), false), "twin: its sliders take the hand");
    store.invoke_release(Field::Exposure.index());
    let rested = Field::Exposure.knob().get(&oni_settings::sidecar::read(&b.roll.join("B00.tif"), false, true).unwrap().settings);
    store.invoke_step(Field::Exposure.index(), 5);
    std::fs::rename(b.root.join("Volumes/T7"), b.root.join("T7 away")).unwrap();
    b.open("C00");
    b.rig.settle();
    let (title, detail) = b.said();
    assert_eq!(title, "“C00.tif” cannot be found");
    assert!(detail.contains("Its disk “T7” is not connected"), "{detail}");
    assert_eq!(w.global::<Absent>().get_folder(), oni_library::paths::normal(&b.root.join("Volumes").to_string_lossy()).as_ref(), "the nearest folder still there");
    let u = w.global::<Unsaved>();
    assert!(u.get_shown(), "the edits a disk gone refused are asked about");
    assert!(u.get_message().contains("Its disk “T7” is not connected."), "{}", u.get_message());
    b.rig.shot("missing-unplugged");
    // The disk back, Retry writes them and the sheet goes.
    std::fs::rename(b.root.join("T7 away"), b.root.join("Volumes/T7")).unwrap();
    u.invoke_answered(1);
    b.rig.settle();
    assert!(!u.get_shown());
    let side = oni_settings::sidecar::url_for(&b.roll.join("B00.tif"));
    let written = oni_settings::sidecar::read(&b.roll.join("B00.tif"), false, true).unwrap().settings;
    assert!(side.exists() && Field::Exposure.knob().get(&written) != rested, "the kept edits reached the sidecar");
    let _ = std::fs::remove_dir_all(&b.root);
}

pub(crate) fn a_heic_says_format_not_supported() {
    let Some(b) = bench("heic", |roll| std::fs::write(roll.join("IMG_0001.HEIC"), b"\0\0\0\x18ftypheic\0\0\0\0mif1heic").unwrap()) else { return };
    let canvas = b.shell.window.global::<Canvas>();
    b.open("IMG_0001");
    b.rig.settle();
    let (title, detail) = b.said();
    assert_eq!(title, "Format not supported (HEIC)");
    assert!(detail.contains("“IMG_0001.HEIC”"), "{detail}");
    assert_eq!(canvas.get_failure(), "", "never “could not be decoded”");
    // Twin: a TIFF of the same roll opens and renders.
    b.open("A00");
    b.rig.until("A00 rendered", Duration::from_secs(120), || b.shell.ctl.state.borrow().renders() >= 1);
    assert_eq!(b.said().0, "");
    let _ = std::fs::remove_dir_all(&b.root);
}
