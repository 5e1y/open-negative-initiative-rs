//! `galleryCellSelfCheck` on the grid's kept tiles: a tile shows its pick, its medal once given and
//! its picture once landed, and a slot re-filled for another photo keeps nothing of the one before.

use oni_app::app::{self, Options, Shell};
use oni_app::Library;
use oni_core::{Archive, EditorStore};
use oni_library::navigator::Computer;
use oni_library::paths;
use slint::ComponentHandle;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

struct Disk {
    root: PathBuf,
}

impl Drop for Disk {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Two folders of the same two names, `One` bright and `Two` dark, so a picture tells its folder.
fn disk() -> Disk {
    let root = std::env::temp_dir().join(format!("oni-app-cell-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for (folder, tone) in [("One", 0.9), ("Two", 0.15)] {
        for name in ["A.tif", "B.tif"] {
            common::pixels_tiff(&root.join("Home").join(folder).join(name), 48, 32, |_, _| [tone, tone, tone]);
        }
    }
    std::fs::create_dir_all(root.join("Volumes")).unwrap();
    Disk { root: PathBuf::from(paths::canonical(&root)) }
}

fn launch(d: &Disk, gpu: &oni_app::render::Device) -> Shell {
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let computer = Computer { home: paths::canonical(&d.root.join("Home")), volumes: paths::canonical(&d.root.join("Volumes")) };
    let options = Options {
        support: d.root.join("Support"),
        caches: d.root.join("Caches"),
        picker: Box::new(|| None),
        computer,
        ..Options::default()
    };
    let shell = Shell::new(store, oni_app::render::Engine::shared(gpu), options).unwrap();
    shell.window.show().unwrap();
    shell
}

fn tile(shell: &Shell, caption: &str) -> oni_app::Tile {
    app::tiles(&shell.window).into_iter().find(|t| t.caption == caption).unwrap_or_else(|| panic!("{caption} is shown"))
}

/// The picture's mean level, 0 to 255; `None` before it lands.
fn level(picture: &slint::Image) -> Option<f64> {
    let pixels = picture.to_rgba8()?;
    let all = pixels.as_slice();
    (!all.is_empty()).then(|| all.iter().map(|p| f64::from(p.r) + f64::from(p.g) + f64::from(p.b)).sum::<f64>() / (3.0 * all.len() as f64))
}

fn open(rig: &common::Rig, shell: &Shell, folder: &Path) {
    let ctl = shell.ctl.clone();
    shell.open_folder(folder);
    let shown = |s: &Shell| ["A", "B"].iter().all(|c| app::tiles(&s.window).iter().any(|t| t.caption == *c && t.picture.size().width > 0));
    rig.until("the folder read and drawn", Duration::from_secs(60), || ctl.state.borrow().read_done() && shown(shell));
    rig.settle();
}

pub(crate) fn a_tile_shows_its_own_pick_medal_and_picture_and_keeps_nothing_of_another() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return println!("SKIPPED gallery cell: {e}"),
    };
    let d = disk();
    let rig = common::rig(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    let shell = launch(&d, &gpu);
    let (one, two) = (d.root.join("Home/One"), d.root.join("Home/Two"));
    open(&rig, &shell, &one);
    let (a, b) = (tile(&shell, "A"), tile(&shell, "B"));
    // The layout's own frames are `gallery.rs`'s check: here, two cells side by side, one row, apart.
    assert!(a.width > 0.0 && a.y == b.y && a.x + a.width <= b.x, "{a:?} against {b:?}");
    assert!(!a.selected && a.medal == 0, "a plain tile: no ring, no medal");
    let bright = level(&a.picture).expect("the thumbnail is laid once it has landed");

    let lib = shell.window.global::<Library>();
    lib.invoke_tile_clicked(a.index, false, false);
    rig.pump();
    rig.key("1");
    rig.pump();
    let picked = tile(&shell, "A");
    assert!(picked.selected && picked.medal != 0, "selected, its ring; and its medal once given");
    assert!(!tile(&shell, "B").selected && tile(&shell, "B").medal == 0, "twin: the other tile has neither");

    open(&rig, &shell, &two);
    let dark = level(&tile(&shell, "A").picture).unwrap();
    assert!((bright - dark).abs() > 40.0, "the two folders' pictures part: {bright} against {dark}");
    let refilled = tile(&shell, "A");
    assert!(!refilled.selected && refilled.medal == 0, "re-filled for another photo, it keeps neither its ring nor its medal");
    // Back to One, whose thumbnails are held already: no landing comes to correct a kept picture.
    open(&rig, &shell, &one);
    let back = level(&tile(&shell, "A").picture).unwrap();
    assert!((back - bright).abs() < (back - dark).abs(), "One's A shows its own picture, never Two's: {back} ({bright} against {dark})");
    rig.shot("gallery-cell");
}

/// A RAW+JPEG pair lays one tile, the RAW's, marked `+JPG`; a HEIC is listed with its format's seal.
pub(crate) fn a_raw_jpeg_pair_is_one_tile_and_a_heic_is_sealed() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return println!("SKIPPED pair tile: {e}"),
    };
    let d = disk();
    let folder = d.root.join("Home/Pair");
    common::pixels_tiff(&folder.join("R.tif"), 48, 32, |_, _| [0.5, 0.5, 0.5]);
    for (name, bytes) in [("P.NEF", &b"a raw"[..]), ("P.JPG", b"its jpeg"), ("Q.HEIC", b"\0\0\0\x18ftypheic")] {
        std::fs::write(folder.join(name), bytes).unwrap();
    }
    let rig = common::rig(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    let shell = launch(&d, &gpu);
    let ctl = shell.ctl.clone();
    shell.open_folder(&folder);
    let sealed = |s: &Shell| app::tiles(&s.window).iter().any(|t| t.caption == "Q" && t.gone_pass == "HEIC");
    rig.until("the folder read and its HEIC looked at", Duration::from_secs(60), || ctl.state.borrow().read_done() && sealed(&shell));
    let captions: Vec<String> = app::tiles(&shell.window).iter().filter(|t| t.width > 0.0).map(|t| t.caption.to_string()).collect();
    assert_eq!(captions.iter().filter(|c| *c == "P").count(), 1, "one tile for the pair: {captions:?}");
    assert_eq!(tile(&shell, "P").companion, "+JPG");
    assert_eq!(tile(&shell, "Q").note, "Format not supported (HEIC)");
    // Twin: a plain TIFF beside them has neither mark.
    let plain = tile(&shell, "R");
    assert!(plain.companion.is_empty() && plain.gone_pass.is_empty(), "{plain:?}");
    // The recalibration notice a RAW raises is closed, so the shot shows the tiles.
    shell.window.global::<oni_app::Notice>().invoke_close();
    rig.settle();
    rig.shot("pair-tile");
}

/// The light of the window's pixel at a point of the grid's document, the three channels summed.
fn light_at(drawn: &(usize, Vec<slint::platform::software_renderer::Rgb565Pixel>), lib: &Library, scale: f32, x: f32, y: f32) -> i32 {
    let (px, py) = (((lib.get_grid_x() + x) * scale) as usize, ((lib.get_grid_y() + y - lib.get_scroll()) * scale) as usize);
    let v = drawn.1[py * drawn.0 + px].0;
    i32::from((v >> 11) & 0x1f) * 8 + i32::from((v >> 5) & 0x3f) * 4 + i32::from(v & 0x1f) * 8
}

/// A selected tile's ring reads on a light photo: the ring on its bounds, then a dark edge between
/// it and the photo, so the ring never melts into a pale sky.
pub(crate) fn a_selected_tiles_ring_reads_on_a_light_photo() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return println!("SKIPPED selected ring: {e}"),
    };
    let d = disk();
    let rig = common::rig(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    let shell = launch(&d, &gpu);
    let lib = shell.window.global::<Library>();
    let scale = rig.window.scale_factor();
    // The folder whose photos the screen shows lighter, whichever way the conversion turns them.
    let mut lightest = (0, d.root.join("Home/One"));
    for folder in [d.root.join("Home/One"), d.root.join("Home/Two")] {
        open(&rig, &shell, &folder);
        rig.settle();
        let a = tile(&shell, "A");
        let photo = light_at(&rig.drawn(), &lib, scale, a.x + a.width / 2.0, a.y + a.height / 2.0);
        if photo > lightest.0 {
            lightest = (photo, folder);
        }
    }
    open(&rig, &shell, &lightest.1);
    // Across the left edge at mid height: the ring's first point, the edge just inside it, the photo.
    let across = |drawn: &(usize, Vec<slint::platform::software_renderer::Rgb565Pixel>), t: &oni_app::Tile| {
        let y = t.y + t.height / 2.0;
        [0.5, 2.5, 6.5].map(|dx| light_at(drawn, &lib, scale, t.x + dx, y))
    };
    let parted = |[ring, edge, photo]: [i32; 3]| photo > 450 && ring > 700 && edge < photo - 150;
    let a = tile(&shell, "A");
    lib.invoke_tile_clicked(a.index, false, false);
    rig.settle();
    let a = tile(&shell, "A");
    assert!(a.selected);
    let picked = across(&rig.drawn(), &a);
    assert!(parted(picked), "the white ring, a dark edge, then the light photo: {picked:?}");
    rig.shot("selected-ring");
    // Twin: the same tile once B is picked shows its light photo to its edge.
    lib.invoke_tile_clicked(tile(&shell, "B").index, false, false);
    rig.settle();
    let a = tile(&shell, "A");
    let left = across(&rig.drawn(), &a);
    assert!(!a.selected && !parted(left) && left[2] > 450, "unselected, neither ring nor edge: {left:?}");
}
