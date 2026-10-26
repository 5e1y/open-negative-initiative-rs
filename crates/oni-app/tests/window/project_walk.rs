//! Navigation inside a project of photos from two folders, as on the archive: the gallery's
//! arrows, ⌘G and the editor's walk follow the project's own order and stay inside it.

use oni_app::app::{self, Options, Shell};
use oni_app::{App, Lab, Library};
use oni_core::{Archive, EditorStore};
use oni_library::project::{self, Project, ProjectsFile};
use oni_settings::Date;
use slint::platform::Key;
use slint::{ComponentHandle, SharedString};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::common;

struct Hand<'a> {
    rig: &'a common::Rig,
    shell: &'a Shell,
}

impl Hand<'_> {
    fn shown(&self) -> Vec<String> {
        let mut tiles = app::tiles(&self.shell.window);
        tiles.sort_by_key(|t| t.index);
        tiles.into_iter().map(|t| t.caption.to_string()).collect()
    }

    fn selected(&self) -> Vec<String> {
        let mut tiles = app::tiles(&self.shell.window);
        tiles.sort_by_key(|t| t.index);
        tiles.into_iter().filter(|t| t.selected).map(|t| t.caption.to_string()).collect()
    }

    /// A plain click on the tile captioned `caption`, after the double-click interval.
    fn pick(&self, caption: &str) {
        let start = Instant::now();
        while start.elapsed() < oni_app::system::double_click() + Duration::from_millis(50) {
            self.rig.pump();
            std::thread::sleep(Duration::from_millis(10));
        }
        let tile = app::tiles(&self.shell.window).into_iter().find(|t| t.caption == caption).unwrap_or_else(|| panic!("{caption} is shown"));
        self.shell.window.global::<Library>().invoke_tile_clicked(tile.index, false, false);
        self.rig.pump();
    }

    fn open(&self) -> String {
        let s = self.shell.ctl.state.borrow();
        Path::new(s.open_path().unwrap_or_default()).file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    }

    fn gallery(&self) -> bool {
        self.shell.window.global::<App>().get_gallery()
    }

    fn frame(&self) -> String {
        self.shell.window.global::<Lab>().get_frame().to_string()
    }

    fn arrow(&self, key: Key) {
        let k: SharedString = key.into();
        self.rig.key(&k);
    }
}

/// Roll A and Roll B, six photos each; Trip holds four from both in its own order, Other two more.
fn scratch() -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!("oni-app-project-walk-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let scans = root.join("Scans");
    // Tones apart, so no two files share a fingerprint.
    for i in 0..6 {
        common::plain_tiff(&scans.join("Roll A").join(format!("A{i:02}.tif")), 96, 64, 0.3 + i as f32 / 40.0);
        common::plain_tiff(&scans.join("Roll B").join(format!("B{i:02}.tif")), 96, 64, 0.65 + i as f32 / 60.0);
    }
    let fingerprint = |name: &str| {
        let path = oni_library::paths::canonical(&scans.join(name));
        oni_library::library::indexed(&path).expect("a readable TIFF").fingerprint
    };
    let at = |minutes: f64| Date::since_reference_date(8e8 + minutes * 60.0);
    let trip = ["Roll B/B02.tif", "Roll A/A03.tif", "Roll B/B04.tif", "Roll A/A01.tif"].map(fingerprint).to_vec();
    let other = ["Roll A/A00.tif", "Roll B/B00.tif"].map(fingerprint).to_vec();
    let projects = vec![Project::new("Other", at(1.0), other), Project::new("Trip", at(2.0), trip)];
    project::save(&root.join("Support"), &ProjectsFile { version: 1, projects }).unwrap();
    (root, scans)
}

fn launch(gpu: &oni_app::render::Device, root: &Path) -> (common::Rig, Shell) {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), oni_app::render::Engine::shared(gpu), options).unwrap();
    rig.window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    (rig, shell)
}

fn opened(rig: &common::Rig, shell: &Shell, folder: &Path) {
    shell.open_folder(folder);
    let ctl = shell.ctl.clone();
    let spelt = oni_library::paths::canonical(folder);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done() && ctl.state.borrow().opened() == Some(spelt.as_str()));
    rig.settle();
}

pub(crate) fn a_project_walks_its_own_order_in_the_gallery_and_the_editor_and_the_archive_its_folder() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: a project's navigation is not exercised"),
    };
    let (root, scans) = scratch();
    let (rig, shell) = launch(&gpu, &root);
    let hand = Hand { rig: &rig, shell: &shell };
    let lib = shell.window.global::<Library>();
    // The whole scans read once so the index holds Roll B, then Roll A alone opened: Trip's B
    // photos stand beyond the folder open.
    opened(&rig, &shell, &scans);
    opened(&rig, &shell, &scans.join("Roll A"));

    lib.invoke_surface_picked(1);
    rig.pump();
    assert_eq!(hand.shown(), ["B02", "A03", "B04", "A01", "A00", "B00"], "Trip, the newer, then Other, each in its own order");

    // The gallery's arrows walk the project's order across both folders; ⇧ extends.
    hand.pick("B02");
    hand.arrow(Key::RightArrow);
    assert_eq!(hand.selected(), ["A03"], "→ from B02 lands on Trip's next, A03");
    hand.arrow(Key::RightArrow);
    assert_eq!(hand.selected(), ["B04"]);
    hand.arrow(Key::LeftArrow);
    assert_eq!(hand.selected(), ["A03"], "← walks back");
    rig.chord(Key::Shift, &SharedString::from(Key::RightArrow));
    assert_eq!(hand.selected(), ["A03", "B04"], "⇧→ extends to B04");

    // ⌘G opens the ringed photo, and the editor's → walks Trip, never Roll A.
    hand.pick("A03");
    rig.chord(Key::Control, "g");
    assert!(!hand.gallery());
    assert_eq!(hand.open(), "A03", "⌘G opens the ringed A03");
    assert_eq!(hand.frame(), "FRAME 02/04", "the screen counts A03 in Trip");
    hand.arrow(Key::RightArrow);
    assert_eq!(hand.open(), "B04", "→ walks to Trip's next, B04 in Roll B");
    hand.arrow(Key::RightArrow);
    assert_eq!(hand.open(), "A01");
    assert_eq!(hand.frame(), "FRAME 04/04");
    hand.arrow(Key::RightArrow);
    assert_eq!(hand.open(), "A01", "Trip's end stops the walk: Other's A00 follows it in the grid only");
    hand.arrow(Key::UpArrow);
    assert_eq!(hand.open(), "B04", "↑ walks back as ←");

    // Back to the gallery, the photo walked to is the selection.
    rig.chord(Key::Control, "g");
    assert!(hand.gallery());
    assert_eq!(hand.selected(), ["B04"], "the gallery comes back on the photo walked to");
    hand.arrow(Key::RightArrow);
    assert_eq!(hand.selected(), ["A01"]);

    // Twin: the same gestures on the archive walk Roll A, the folder open.
    lib.invoke_surface_picked(0);
    rig.pump();
    assert_eq!(hand.shown(), ["A00", "A01", "A02", "A03", "A04", "A05"]);
    hand.pick("A03");
    hand.arrow(Key::RightArrow);
    assert_eq!(hand.selected(), ["A04"], "twin: → on the archive lands on the folder's next");
    hand.arrow(Key::LeftArrow);
    rig.chord(Key::Control, "g");
    assert_eq!(hand.open(), "A03");
    assert_eq!(hand.frame(), "FRAME 04/06", "twin: the screen counts A03 in Roll A");
    hand.arrow(Key::RightArrow);
    assert_eq!(hand.open(), "A04", "twin: → in the editor walks the folder, not Trip");
    rig.chord(Key::Control, "g");
    assert_eq!(hand.selected(), ["A04"]);

    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
