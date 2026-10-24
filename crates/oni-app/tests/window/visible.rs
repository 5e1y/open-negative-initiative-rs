//! What is out of reach, drawn: a favourite gone fades with its words and a greyed menu, an opened
//! folder away is said by a banner and never as empty, a missing tile greys its picture alone, a
//! triplet missing a pass says which. The disks are folders of the check's own, never `/Volumes`.

use oni_app::app::{self, Options, Shell};
use oni_app::{FolderLine, Library, Navigator, Reach};
use oni_core::{Archive, EditorStore};
use oni_library::favourites::Favourites;
use oni_library::navigator::Computer;
use oni_library::paths;
use oni_settings::sidecar::{self, WriteOptions};
use slint::{ComponentHandle, Model};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::common;

struct Disk {
    root: PathBuf,
    support: PathBuf,
    caches: PathBuf,
    computer: Computer,
}

/// A home and a volumes folder in the check's own scratch, each file a TIFF of `tone`.
fn disk(label: &str, files: &[(&str, f32)]) -> Disk {
    let root = std::env::temp_dir().join(format!("oni-app-visible-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for (file, tone) in files {
        common::pixels_tiff(&root.join(file), 48, 32, |x, _| [0.9 * *tone, 0.3, 0.1 + 0.01 * x as f32]);
    }
    std::fs::create_dir_all(root.join("Home")).unwrap();
    std::fs::create_dir_all(root.join("Volumes")).unwrap();
    let root = PathBuf::from(paths::canonical(&root));
    let computer = Computer { home: paths::canonical(&root.join("Home")), volumes: paths::canonical(&root.join("Volumes")) };
    Disk { support: root.join("Support"), caches: root.join("Caches"), root, computer }
}

impl Disk {
    fn launch(&self, gpu: &oni_app::render::Device) -> Shell {
        let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
        let options = Options {
            support: self.support.clone(),
            caches: self.caches.clone(),
            picker: Box::new(|| None),
            computer: self.computer.clone(),
            ..Options::default()
        };
        let shell = Shell::new(Box::new(store), oni_app::render::Engine::shared(gpu), options).unwrap();
        shell.window.show().unwrap();
        shell
    }

    fn path(&self, rel: &str) -> String {
        paths::normal(&self.root.join(rel).to_string_lossy()).into_owned()
    }
}

fn rig() -> common::Rig {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    rig
}

fn gpu(what: &str) -> Option<oni_app::render::Device> {
    oni_app::render::Device::open().map_err(|e| eprintln!("{e}: {what} is not exercised")).ok()
}

fn lines(w: &oni_app::MainWindow) -> Vec<FolderLine> {
    let rows = w.global::<Navigator>().get_rows();
    (0..rows.row_count()).filter_map(|i| rows.row_data(i)).collect()
}

fn row(w: &oni_app::MainWindow, title: &str) -> Option<FolderLine> {
    lines(w).into_iter().find(|l| l.title == title)
}

fn at(w: &oni_app::MainWindow, title: &str) -> i32 {
    lines(w).iter().position(|l| l.title == title).unwrap_or_else(|| panic!("{title} is not a row")) as i32
}

fn menu(w: &oni_app::MainWindow, title: &str) -> Vec<(String, bool)> {
    let n = w.global::<Navigator>();
    assert!(n.invoke_row_menu(at(w, title)));
    let m = n.get_menu_lines();
    (0..m.row_count()).filter_map(|i| m.row_data(i)).map(|l| (l.text.to_string(), l.enabled)).collect()
}

fn tile(shell: &Shell, caption: &str) -> oni_app::Tile {
    app::tiles(&shell.window).into_iter().find(|t| t.caption == caption).unwrap_or_else(|| panic!("{caption} is shown"))
}

/// The banner over the gallery: shown, its title and its words.
fn banner(shell: &Shell) -> (bool, String, String) {
    let l = shell.window.global::<Library>();
    (l.get_banner_shown(), l.get_banner_title().to_string(), l.get_banner_message().to_string())
}

fn empty_title(shell: &Shell) -> String {
    shell.window.global::<Library>().get_empty_title().to_string()
}

pub(crate) fn a_favourite_moved_away_on_the_home_fades_and_its_menu_greys() {
    let Some(gpu) = gpu("a favourite gone") else { return };
    let d = disk("favourite", &[("Home/Scans/Roll 12/A.tif", 0.5), ("Home/Keep/B.tif", 0.7)]);
    let (gone, keep) = (d.path("Home/Scans/Roll 12"), d.path("Home/Keep"));
    std::fs::create_dir_all(&d.support).unwrap();
    Favourites { version: 1, paths: vec![gone.clone(), keep.clone()] }.save(&d.support).unwrap();
    // Moved in the Finder while ONI was closed, its disk the home's, which always answers.
    std::fs::rename(d.root.join("Home/Scans/Roll 12"), d.root.join("Home/Scans/Roll 12 old")).unwrap();
    let rig = rig();
    let shell = d.launch(&gpu);
    let w = &shell.window;
    let n = w.global::<Navigator>();
    n.invoke_favourites_toggled();
    rig.pump();
    rig.until("the favourite found gone", Duration::from_secs(10), || row(w, "Roll 12").is_some_and(|r| r.reach == Reach::Missing));
    let r = row(w, "Roll 12").unwrap();
    assert!(!r.present && !r.chevron, "faded, and it promises no folder inside");
    assert_eq!(r.caption.as_str(), "Not found");
    assert!(r.help.starts_with(&gone) && r.help.contains("moved, renamed or deleted"), "{}", r.help);
    assert_eq!(menu(w, "Roll 12"), [
        ("Open".to_string(), true),
        ("Remove from Favourites".to_string(), true),
        ("Locate…".to_string(), true),
        ("Rename…".to_string(), false),
        ("Make a Roll".to_string(), false),
        ("New Folder Inside".to_string(), false),
        ("Show the Parent Folder".to_string(), true),
        ("Remove from Library…".to_string(), true),
    ]);
    // Twin: the favourite there stays whole, its menu whole.
    rig.until("Keep listed", Duration::from_secs(10), || row(w, "Keep").is_some_and(|r| r.present));
    let k = row(w, "Keep").unwrap();
    assert_eq!((k.reach, k.caption.as_str(), k.help.as_str()), (Reach::Here, "", ""));
    assert!(menu(w, "Keep").iter().all(|(_, enabled)| *enabled));
    assert_eq!(menu(w, "Keep").last().unwrap().0, "Show in Finder");
    rig.settle();
    rig.shot("visible-favourite-gone");

    // A click shows its state in the gallery, never nothing.
    n.invoke_row_clicked(at(w, "Roll 12"));
    rig.pump();
    let ctl = shell.ctl.clone();
    rig.until("Roll 12 opened", Duration::from_secs(20), || ctl.state.borrow().opened() == Some(gone.as_str()) && ctl.state.borrow().read_done());
    rig.until("its banner", Duration::from_secs(10), || banner(&shell).0);
    let (_, title, message) = banner(&shell);
    assert_eq!(title, "Folder Not Found");
    assert!(message.contains("“Roll 12” is no longer in “Scans”"), "{message}");
    assert_eq!(empty_title(&shell), "", "a folder gone is not an empty folder");
    let keys = || {
        let k = w.global::<Library>().get_banner_keys();
        (0..k.row_count()).filter_map(|i| k.row_data(i)).map(|k| k.to_string()).collect::<Vec<_>>()
    };
    assert_eq!(keys(), ["LOCATE…", "REMOVE FROM FAVOURITES"], "never read, it holds nothing to forget");
    rig.settle();
    rig.shot("visible-favourite-gone-gallery");
    // Its key takes the favourite away and leaves the banner with Locate alone.
    w.global::<Library>().invoke_banner_acted(1);
    rig.pump();
    assert!(!Favourites::load(&d.support).unwrap().paths.contains(&gone));
    assert_eq!(keys(), ["LOCATE…"]);
    drop((ctl, shell));
    let _ = std::fs::remove_dir_all(&d.root);
}

pub(crate) fn an_opened_folder_gone_is_not_an_empty_folder() {
    let Some(gpu) = gpu("a folder away") else { return };
    let d = disk("away", &[("Volumes/T7/Roll 12/A.tif", 0.5), ("Volumes/T7/Roll 12/B.tif", 0.8)]);
    std::fs::create_dir_all(d.root.join("Home/Empty")).unwrap();
    let rig = rig();
    let shell = d.launch(&gpu);
    let ctl = shell.ctl.clone();
    let roll = d.path("Volumes/T7/Roll 12");
    shell.open_folder(Path::new(&roll));
    rig.until("Roll 12 read and drawn", Duration::from_secs(60), || ctl.state.borrow().read_done() && ctl.state.borrow().loaded() >= 2);
    assert!(!banner(&shell).0, "a folder there has no banner");

    // Its disk unplugged while it shows: the tiles kept, the banner saying why, the count below.
    std::fs::rename(d.root.join("Volumes/T7"), d.root.join("T7 away")).unwrap();
    rig.until("T7 gone", Duration::from_secs(10), || banner(&shell).0);
    let (_, title, message) = banner(&shell);
    assert_eq!(title, "T7 Not Connected");
    assert!(message.starts_with("“Roll 12” is on “T7”, which is not connected.") && message.contains("last thumbnails"), "{message}");
    assert_eq!(shell.window.global::<Library>().get_banner_reach(), Reach::Offline);
    assert!(tile(&shell, "A").missing && tile(&shell, "B").missing);
    assert_eq!(empty_title(&shell), "");
    let status = shell.window.global::<Library>().get_status().to_string();
    assert!(status.ends_with("· 2 originals missing"), "{status}");
    rig.settle();
    rig.shot("visible-disk-away");

    // Gone from a disk that answers and never read: a banner, never "No Photos".
    let gone = d.path("Home/Gone");
    shell.open_folder(Path::new(&gone));
    rig.until("Gone read", Duration::from_secs(20), || ctl.state.borrow().opened() == Some(gone.as_str()) && ctl.state.borrow().read_done());
    rig.until("its banner", Duration::from_secs(10), || banner(&shell).1 == "Folder Not Found");
    assert_eq!(empty_title(&shell), "", "no word of an empty folder");
    assert!(!banner(&shell).2.contains("thumbnails"), "none kept, none promised");

    // Twin: a real empty folder says No Photos, and no banner.
    let empty = d.path("Home/Empty");
    shell.open_folder(Path::new(&empty));
    rig.until("Empty read", Duration::from_secs(20), || ctl.state.borrow().opened() == Some(empty.as_str()) && ctl.state.borrow().read_done());
    rig.until("its empty state", Duration::from_secs(10), || empty_title(&shell) == "No Photos");
    assert!(!banner(&shell).0);
    drop((ctl, shell));
    let _ = std::fs::remove_dir_all(&d.root);
}

/// The window's pixel at a point of the grid's document, as RGB.
fn pixel(rig: &common::Rig, drawn: &(usize, Vec<slint::platform::software_renderer::Rgb565Pixel>), shell: &Shell, x: f32, y: f32) -> [i32; 3] {
    let lib = shell.window.global::<Library>();
    let scale = rig.window.scale_factor();
    let (px, py) = (((lib.get_grid_x() + x) * scale) as usize, ((lib.get_grid_y() + y - lib.get_scroll()) * scale) as usize);
    let v = drawn.1[py * drawn.0 + px].0;
    [i32::from((v >> 11) & 0x1f) * 8, i32::from((v >> 5) & 0x3f) * 4, i32::from(v & 0x1f) * 8]
}

fn grey(picture: &slint::Image) -> bool {
    picture.to_rgba8().is_some_and(|p| p.as_slice().iter().all(|c| c.r == c.g && c.g == c.b))
}

pub(crate) fn a_missing_tile_is_grey_on_its_own_layer() {
    let Some(gpu) = gpu("a missing tile") else { return };
    let d = disk("tile", &[("Home/Scans/A.tif", 0.9), ("Home/Scans/B.tif", 0.9)]);
    for name in ["A.tif", "B.tif"] {
        sidecar::set_label(oni_settings::Medal::Gold, &d.root.join("Home/Scans").join(name), false).unwrap();
    }
    let rig = rig();
    let shell = d.launch(&gpu);
    let ctl = shell.ctl.clone();
    // B taken away stays a missing tile: no watch reads the folder again behind the check.
    ctl.set_watching(false);
    shell.open_folder(&d.root.join("Home/Scans"));
    rig.until("Scans read and drawn", Duration::from_secs(60), || ctl.state.borrow().read_done() && ctl.state.borrow().loaded() >= 2);
    rig.until("both pictures", Duration::from_secs(20), || ["A", "B"].iter().all(|c| tile(&shell, c).picture.size().width > 0));
    assert!(!grey(&tile(&shell, "B").picture), "a colour thumbnail to begin with");

    std::fs::remove_file(d.root.join("Home/Scans/B.tif")).unwrap();
    ctl.probe_presence();
    rig.until("B found missing", Duration::from_secs(10), || tile(&shell, "B").missing);
    let (a, b) = (tile(&shell, "A"), tile(&shell, "B"));
    assert!(grey(&b.picture), "its picture greyed, as `TileLayers` filters it");
    assert!(!grey(&a.picture), "twin: the photo there keeps its colour");
    assert_eq!((b.away, a.away), (Reach::Missing, Reach::Here));
    assert_eq!(b.note.as_str(), format!("Original not found at {}", d.path("Home/Scans/B.tif")));
    assert_eq!(a.note.as_str(), "");
    rig.settle();
    let drawn = rig.drawn();
    // The medal and the rounded corner drawn alike on both: only the picture fades.
    let medal = |t: &oni_app::Tile| pixel(&rig, &drawn, &shell, t.x + 14.0, t.y + t.height - 18.0);
    let corner = |t: &oni_app::Tile| pixel(&rig, &drawn, &shell, t.x + 1.0, t.y + 1.0);
    let middle = |t: &oni_app::Tile| pixel(&rig, &drawn, &shell, t.x + t.width / 2.0, t.y + t.height / 2.0);
    let near = |p: [i32; 3], q: [i32; 3]| p.iter().zip(&q).all(|(x, y)| (x - y).abs() <= 8);
    assert!(near(medal(&a), medal(&b)), "the medal whole: {:?} against {:?}", medal(&a), medal(&b));
    assert!(near(corner(&a), corner(&b)), "the corner's mask whole: {:?} against {:?}", corner(&a), corner(&b));
    // Twin: the picture itself is drawn otherwise.
    assert!(!near(middle(&a), middle(&b)), "the picture faded: {:?} against {:?}", middle(&a), middle(&b));
    rig.shot("visible-missing-tile");
    drop((ctl, shell));
    let _ = std::fs::remove_dir_all(&d.root);
}

/// A triplet beside its lead: the lead's sidecar names its passes, each pass names its lead.
fn triplet(d: &Disk, lead: &str, red: &str, blue: &str) {
    let folder = d.root.join("Home/Passes");
    let passes = oni_settings::frame::RgbPasses { red: red.into(), green: lead.into(), blue: blue.into(), registration: None };
    let led = oni_settings::PipelineSettings { rgb_passes: Some(passes), ..Default::default() };
    sidecar::write(&led, &folder.join(lead), WriteOptions::default()).unwrap();
    for pass in [red, blue] {
        let of = oni_settings::PipelineSettings { pass_of: Some(lead.into()), ..Default::default() };
        sidecar::write(&of, &folder.join(pass), WriteOptions::default()).unwrap();
    }
}

pub(crate) fn a_triplet_with_a_pass_gone_is_marked() {
    let Some(gpu) = gpu("a pass gone") else { return };
    let files: Vec<(String, f32)> = ["P0", "P1", "P2", "Q0", "Q1", "Q2"].iter().map(|n| (format!("Home/Passes/{n}.tif"), 0.6)).collect();
    let files: Vec<(&str, f32)> = files.iter().map(|(f, t)| (f.as_str(), *t)).collect();
    let d = disk("passes", &files);
    triplet(&d, "P0.tif", "P1.tif", "P2.tif");
    triplet(&d, "Q0.tif", "Q1.tif", "Q2.tif");
    let rig = rig();
    let shell = d.launch(&gpu);
    let ctl = shell.ctl.clone();
    shell.open_folder(&d.root.join("Home/Passes"));
    rig.until("Passes read", Duration::from_secs(60), || ctl.state.borrow().read_done() && app::tiles(&shell.window).len() == 2);
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(500) {
        rig.pump();
    }
    let (p, q) = (tile(&shell, "P0"), tile(&shell, "Q0"));
    assert!(p.rgb && q.rgb && p.gone_pass.is_empty() && q.gone_pass.is_empty(), "each triplet whole, its RGB seal");

    std::fs::remove_file(d.root.join("Home/Passes/P1.tif")).unwrap();
    ctl.probe_presence();
    rig.until("P0's red pass found gone", Duration::from_secs(10), || !tile(&shell, "P0").gone_pass.is_empty());
    let p = tile(&shell, "P0");
    assert_eq!(p.gone_pass.as_str(), "R MISSING");
    assert_eq!(p.note.as_str(), "Its red pass “P1.tif” is missing beside it.");
    assert!(!p.missing, "the lead itself is there");
    // Twin: the whole triplet keeps its plain seal.
    let q = tile(&shell, "Q0");
    assert!(q.rgb && q.gone_pass.is_empty() && q.note.is_empty());
    rig.settle();
    rig.shot("visible-pass-gone");
    // Back beside it, the seal is whole again.
    common::plain_tiff(&d.root.join("Home/Passes/P1.tif"), 48, 32, 0.5);
    ctl.probe_presence();
    rig.until("the pass back", Duration::from_secs(10), || tile(&shell, "P0").gone_pass.is_empty());
    drop((ctl, shell));
    let _ = std::fs::remove_dir_all(&d.root);
}
