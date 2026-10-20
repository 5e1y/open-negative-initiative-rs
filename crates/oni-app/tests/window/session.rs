//! What one session leaves the next on the headless window: the gallery's tile size and the key
//! clicks, under Swift's `@AppStorage` keys, set from the menu or from the Settings sheet.

use oni_app::app::{Options, Shell};
use oni_app::preferences::{self, Preferences};
use oni_app::{App, AppSettings, Bar, Menus};
use oni_core::{Archive, EditorStore};
use slint::ComponentHandle;
use slint::platform::WindowEvent;
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

pub(crate) fn the_tile_size_and_the_muted_clicks_outlast_the_session() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the session is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-session-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support) = (root.join("Scans"), root.join("Support"));
    common::linear_tiff(&folder.join("A.tif"), 96, 64, 0.6);
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let shell = |support: &PathBuf| {
        let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
        let options = Options { support: support.clone(), caches: root.join("Caches"), picker: Box::new(|| None), ..Options::default() };
        let shell = Shell::new(Box::new(store), oni_app::render::Engine::shared(&gpu), options).unwrap();
        rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
        rig.window.set_size(slint::PhysicalSize::new(1280, 800));
        shell.window.show().unwrap();
        shell.open_folder(&folder);
        rig.until("the folder read", Duration::from_secs(30), || shell.ctl.state.borrow().read_done());
        shell
    };

    let first = shell(&support);
    assert!(first.window.global::<App>().get_gallery());
    assert!(first.window.global::<Menus>().get_live().smaller, "the film rung has a denser one");
    first.ctl.act("smaller");
    // The header's gear opens the sheet; its switches are the menu's own preferences.
    first.window.global::<Bar>().invoke_act("settings".into());
    rig.pump();
    let sheet = first.window.global::<AppSettings>();
    assert!(sheet.get_shown() && first.window.get_sheet_shown());
    assert_eq!((sheet.get_clicks(), sheet.get_half_size()), (false, false), "silent and whole by default");
    sheet.invoke_flip("toggle-sound".into());
    sheet.invoke_flip("half-size".into());
    rig.pump();
    assert_eq!((sheet.get_clicks(), sheet.get_half_size()), (true, true));
    assert_eq!(first.window.global::<Menus>().get_titles().sound.as_str(), "Mute Key Clicks");
    rig.key("\u{1b}");
    assert!(!sheet.get_shown(), "Escape closes the sheet");
    let kept = Preferences::load(&support);
    assert_eq!((kept.int(preferences::GALLERY_COLUMNS), kept.bool(preferences::SOUND_MUTED)), (Some(12), Some(false)));
    assert_eq!(kept.bool(preferences::HALF_SIZE_PREVIEWS), Some(true));
    drop(first);

    let second = shell(&support);
    let menus = second.window.global::<Menus>();
    assert!(!menus.get_live().smaller, "the next session opens on the dense rung");
    assert_eq!(menus.get_titles().sound.as_str(), "Mute Key Clicks");
    second.ctl.act("settings");
    assert!(second.window.global::<AppSettings>().get_clicks(), "the sheet reads the kept choice");
    drop(second);

    // Twin: a fresh support folder opens silent, the owner's default where Swift clicks.
    let fresh = shell(&root.join("Fresh"));
    let menus = fresh.window.global::<Menus>();
    assert!(menus.get_live().smaller);
    assert_eq!(menus.get_titles().sound.as_str(), "Unmute Key Clicks");
    let _ = std::fs::remove_dir_all(&root);
}
