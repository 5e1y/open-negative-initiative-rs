//! The navigator marks a roll still to prepare as its card does: none measured nor a slide; a roll
//! measured, a slide's or a folder of folders shows none, and Reset the Roll marks its row at once.

use oni_app::app::{self, Options, Shell};
use oni_app::{Act, Confirm, FolderLine, Library, Navigator};
use oni_core::{Archive, EditorStore};
use oni_settings::frame::RollReference;
use oni_settings::sidecar::{self, WriteOptions};
use oni_settings::{ConversionMode, Id, PipelineSettings};
use slint::platform::WindowEvent;
use slint::{ComponentHandle, Model};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common;

/// A photo with its sidecar: measured with its roll, a slide, or none written.
fn photo(path: &Path, sidecar: Option<ConversionMode>, measured: bool) {
    common::linear_tiff(path, 64, 48, 0.5);
    if let Some(mode) = sidecar {
        let mut s = PipelineSettings::rest(mode, false);
        if measured {
            s.roll_reference = Some(RollReference { id: Id::new(), stops: [0.0; 3], black: [0.0; 3], white: [1.0; 3], base: None, median: None });
        }
        sidecar::write(&s, path, WriteOptions::default()).unwrap();
    }
}

/// What assistive tools read of a row shown in the column, by its title.
fn described(window: &impl ComponentHandle, title: &str) -> Option<String> {
    i_slint_backend_testing::ElementHandle::find_by_accessible_label(window, title)
        .find(|e| e.accessible_role() == Some(i_slint_backend_testing::AccessibleRole::ListItem))
        .and_then(|e| e.accessible_description())
        .map(|d| d.to_string())
}

pub(crate) fn a_roll_still_to_prepare_is_marked_in_the_navigator() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the navigator's marks are not exercised"),
    };
    let made = std::env::temp_dir().join(format!("oni-app-unprepared-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&made);
    let root = PathBuf::from(oni_library::paths::canonical(&{
        std::fs::create_dir_all(&made).unwrap();
        made.clone()
    }));
    let (home, volumes) = (root.join("Home"), root.join("Volumes"));
    std::fs::create_dir_all(volumes).unwrap();
    let scans = home.join("Scans");
    photo(&scans.join("Ready/A.tif"), Some(ConversionMode::Negative), true);
    photo(&scans.join("Raw/B.tif"), None, false);
    photo(&scans.join("Slide/C.tif"), Some(ConversionMode::Positive), false);
    std::fs::create_dir_all(scans.join("Bare/Inner")).unwrap();
    let p = |path: &Path| oni_library::paths::normal(&path.to_string_lossy()).into_owned();
    let rig = common::rig(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let computer = oni_library::navigator::Computer { home: p(&home), volumes: p(&root.join("Volumes")) };
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), computer, ..Options::default() };
    let shell = Shell::new(Box::new(store), oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 2.0 });
    rig.window.set_size(slint::PhysicalSize::new(2560, 1600));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let n = shell.window.global::<Navigator>();
    let lines = || -> Vec<FolderLine> { n.get_rows().iter().collect() };
    let line = |title: &str| lines().into_iter().find(|l| l.title == title);
    shell.add(&scans);
    rig.until("Scans read", Duration::from_secs(30), || ctl.state.borrow().read_done() && line("Scans").is_some_and(|l| l.opened));
    n.invoke_row_folded(lines().iter().position(|l| l.title == "Scans").unwrap() as i32);
    rig.until("the rolls listed", Duration::from_secs(10), || ["Ready", "Slide", "Bare"].iter().all(|t| line(t).is_some()) && line("Raw").is_some_and(|l| l.unprepared));
    rig.settle();

    assert!(line("Raw").unwrap().unprepared, "photos and no measure: the roll asks for Prepare Roll");
    assert_eq!(described(&shell.window, "Raw").as_deref(), Some("Not prepared yet"), "the row says so to assistive tools");
    rig.shot("unprepared-row");
    // Twins: a measured roll, a slide's and a folder of folders carry no mark.
    for title in ["Ready", "Slide", "Bare"] {
        assert!(!line(title).unwrap().unprepared, "{title} asks for nothing");
        assert_eq!(described(&shell.window, title).as_deref().unwrap_or(""), "", "{title} says nothing");
    }

    // Reset the Roll takes Ready's measure away: its row is marked once the sidecar is written.
    let ready = p(&scans.join("Ready/A.tif"));
    let tile = app::tiles(&shell.window).into_iter().find(|t| t.caption == "A").expect("Ready's photo is shown");
    shell.window.global::<Library>().invoke_tile_clicked(tile.index, false, false);
    rig.pump();
    ctl.act(Act::ResetToDefault);
    let confirm = shell.window.global::<Confirm>();
    rig.until("Reset the Roll's confirm", Duration::from_secs(10), || confirm.get_shown());
    confirm.invoke_answered(true);
    rig.landed(&ctl);
    assert!(sidecar::read(Path::new(&ready), false, true).unwrap().settings.roll_reference.is_none(), "the reset took the measure");
    rig.until("Ready marked", Duration::from_secs(10), || line("Ready").is_some_and(|l| l.unprepared));
    assert!(!line("Slide").unwrap().unprepared, "twin: the slide's roll is untouched");
    drop(ctl);
    drop(shell);
    let _ = std::fs::remove_dir_all(&made);
}
