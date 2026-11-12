//! The rating filter in the library column: picked on its select there, it keeps one medal in the
//! column's tree and in the grid alike, and Select All then picks what it shows, which Export takes.

use i_slint_backend_testing::AccessibleRole as Role;
use oni_app::app::{Ctl, Options, Shell, export_card};
use oni_app::{Act, App, Bar, Exporting, Library, MainWindow};
use oni_core::{Archive, EditorStore};
use slint::platform::Key;
use slint::{ComponentHandle, Model, SharedString};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use crate::common::{self, Rig};

/// Two rolls of two frames, A and C wearing Gold, in a window at the app's size.
struct Roll {
    shell: Shell,
    rig: Rig,
    root: PathBuf,
}

impl Roll {
    fn new(name: &str) -> Option<Roll> {
        let gpu = match oni_app::render::Device::open() {
            Ok(gpu) => gpu,
            Err(e) => {
                eprintln!("{e}: the column's filter is not exercised");
                return None;
            }
        };
        let root = std::env::temp_dir().join(format!("oni-app-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let folder = root.join("Scans");
        for (i, f) in ["Roll 1/A.tif", "Roll 1/B.tif", "Roll 2/C.tif", "Roll 2/D.tif"].iter().enumerate() {
            common::plain_tiff(&folder.join(f), 96, 64, 0.2 + 0.15 * i as f32);
        }
        let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
        std::fs::create_dir_all(&shots).unwrap();
        let rig = common::rig(shots);
        let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
        let engine = oni_app::render::Engine::shared(&gpu);
        let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), ..Options::default() };
        let shell = Shell::new(store, engine, options).unwrap();
        rig.window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
        rig.window.set_size(slint::PhysicalSize::new(1280, 800));
        shell.window.show().unwrap();
        shell.open_folder(&folder);
        let ctl = shell.ctl.clone();
        rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
        let roll = Roll { shell, rig, root };
        for gold in ["A", "C"] {
            roll.lib().invoke_tile_clicked(roll.tile(gold), false, false);
            roll.rig.key("1");
            roll.rig.landed(&roll.ctl());
        }
        Some(roll)
    }

    fn w(&self) -> &MainWindow {
        &self.shell.window
    }

    fn ctl(&self) -> Rc<Ctl> {
        self.shell.ctl.clone()
    }

    fn lib(&self) -> Library<'_> {
        self.w().global::<Library>()
    }

    fn tile(&self, name: &str) -> i32 {
        oni_app::app::tiles(self.w()).into_iter().find(|t| t.caption == name).unwrap_or_else(|| panic!("{name}'s tile")).index
    }

    /// The photos the grid shows, by name, sorted.
    fn shown(&self) -> Vec<String> {
        sorted(self.ctl().state.borrow().order().iter().map(|p| oni_library::paths::name(p)))
    }

    /// The column tree's photo rows, by name, sorted.
    fn strip(&self) -> Vec<String> {
        sorted(self.lib().get_rows().iter().filter(|r| r.kind == 2).map(|r| r.title.to_string()))
    }

    /// The column's filter, named by the photos it keeps: the one select so named left of the grid.
    fn filter(&self, kept: &str) -> common::Placed {
        let column: Vec<_> = common::placed(self.w(), Role::Button, kept).into_iter().filter(|p| p.x + p.width <= 260.0 && p.y > 100.0).collect();
        assert_eq!(column.len(), 1, "one {kept:?} select in the column: {column:?}");
        column[0]
    }

    /// The filter's menu dropped from its select, then the line keeping `medal` clicked.
    fn pick(&self, shown: &str, medal: &str, shot: &str) {
        common::click(&self.rig, self.w(), self.filter(shown).middle());
        self.rig.settle();
        self.rig.shot(shot);
        common::click(&self.rig, self.w(), self.line(medal).middle());
        self.rig.settle();
    }

    /// The menu's line so named, in the window's points: as wide as the menu, never the status bar's
    /// medal key of the same name, which the search finds too.
    fn line(&self, name: &str) -> common::Placed {
        let inner = i_slint_core::window::WindowInner::from_pub(self.w().window());
        let corner = match inner.active_popups().last().map(|p| &p.location) {
            Some(i_slint_core::window::PopupWindowLocation::ChildWindow(at)) => *at,
            _ => panic!("no menu shown, so no {name:?} line"),
        };
        let lines: Vec<_> = common::placed(self.w(), Role::Button, name).into_iter().filter(|p| p.width > 100.0).collect();
        assert_eq!(lines.len(), 1, "one {name:?} line: {lines:?}");
        common::Placed { x: corner.x + lines[0].x, y: corner.y + lines[0].y, ..lines[0] }
    }

    /// Select All, then Export: the frames its sheet opens on, by name.
    fn exported(&self) -> Vec<String> {
        self.w().global::<Bar>().invoke_act(Act::SelectAll);
        self.rig.settle();
        self.rig.chord(Key::Control, "e");
        self.rig.settle();
        assert!(self.w().global::<Exporting>().get_shown(), "⌘E opens the export surface");
        let names = sorted(export_card::card(&self.ctl()).targets.iter().map(|t| t.path.file_name().unwrap().to_string_lossy().into_owned()));
        self.ctl().leave_export();
        self.rig.settle();
        names
    }
}

fn sorted(names: impl Iterator<Item = String>) -> Vec<String> {
    let mut names: Vec<String> = names.collect();
    names.sort();
    names
}

impl Drop for Roll {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Beside B, which wears no medal, Gold picked on the column's select keeps A and C in the column and
/// the grid; Select All picks both rolls' Gold frames, and the export sheet takes exactly those.
pub(crate) fn the_column_s_filter_keeps_gold_beside_a_photo_and_select_all_exports_it() {
    let Some(roll) = Roll::new("sidebar-filter-editor") else { return };
    let (lib, ctl) = (roll.lib(), roll.ctl());
    lib.invoke_tile_opened(roll.tile("B"));
    roll.rig.until("B rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    roll.rig.settle();
    assert!(!roll.w().global::<App>().get_gallery());
    assert_eq!(roll.strip(), ["A", "B", "C", "D"], "every photo before the filter");

    roll.pick("All", "Gold", "sidebar-filter-editor-menu");
    assert_eq!(lib.get_filter_medal(), 1);
    assert_eq!((roll.strip(), roll.shown()), (vec!["A".to_string(), "C".to_string()], vec!["A.tif".to_string(), "C.tif".to_string()]), "one state for the column and the grid");
    roll.filter("Gold");
    println!("shot {}", roll.rig.shot("sidebar-filter-editor-gold").display());
    assert_eq!(roll.exported(), ["A.tif", "C.tif"], "Select All picks what the filter shows, both rolls'");
    println!("shot {}", roll.rig.shot("sidebar-filter-editor-together").display());

    // Twin: every photo shown again, Select All and Export take all four.
    roll.pick("Gold", "All Photos", "sidebar-filter-editor-back");
    assert_eq!((lib.get_filter_medal(), roll.strip().len()), (0, 4));
    assert_eq!(roll.exported().len(), 4);
}

/// In the gallery the column's select heads it under the tabs, above the folders; Gold keeps A and C
/// in the grid, Select All picks both, the export sheet takes exactly those, and the keys reach its lines.
pub(crate) fn the_column_s_filter_keeps_gold_in_the_gallery_and_select_all_exports_it() {
    let Some(roll) = Roll::new("sidebar-filter-gallery") else { return };
    let lib = roll.lib();
    assert!(roll.w().global::<App>().get_gallery());
    let (head, folders) = (roll.filter("All Photos"), common::the(roll.w(), Role::Button, "Hide Computer"));
    assert!(head.y + head.height <= folders.y, "the filter heads the column, above the folders: {head:?} {folders:?}");

    roll.pick("All Photos", "Gold", "sidebar-filter-gallery-menu");
    assert_eq!(roll.shown(), ["A.tif", "C.tif"]);
    println!("shot {}", roll.rig.shot("sidebar-filter-gallery-gold").display());
    assert_eq!(roll.exported(), ["A.tif", "C.tif"], "Select All picks every photo the filter shows, not C's roll alone");

    // Twin: All Photos picked by the keys, the arrows walking the menu to its first line and Return choosing.
    common::click(&roll.rig, roll.w(), roll.filter("Gold").middle());
    roll.rig.settle();
    for _ in 0..4 {
        roll.rig.key(&SharedString::from(Key::UpArrow));
    }
    roll.rig.key(&SharedString::from(Key::Return));
    roll.rig.settle();
    assert_eq!((lib.get_filter_medal(), roll.shown().len()), (0, 4), "the first line keeps every photo");
    lib.invoke_tile_clicked(roll.tile("C"), false, false);
    roll.rig.pump();
    assert_eq!(roll.exported(), ["C.tif", "D.tif"], "with no filter Select All completes C's roll alone");
}
