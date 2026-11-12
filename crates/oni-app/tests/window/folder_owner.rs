//! The folder shown owns its rows: another asked keeps it whole until its own read lands, then
//! replaces all of it at once.

use crate::freshness::{disk, gpu, names, opened, rig};
use oni_app::Library;
use oni_library::paths;
use slint::ComponentHandle;
use std::path::Path;
use std::time::{Duration, Instant};

pub(crate) fn a_folder_asked_keeps_the_one_shown_whole_until_its_read_lands() {
    let Some(gpu) = gpu("a folder asked while another shows") else { return };
    let d = disk("owner", &["Home/Scans/A/A1.tif", "Home/Scans/A/A2.tif", "Home/Scans/A/A3.tif", "Home/Scans/B/B1.tif"]);
    let rig = rig();
    let shell = d.launch(&gpu);
    let ctl = shell.ctl.clone();
    ctl.set_watching(false);
    let (a, b) = (d.path("Home/Scans/A"), d.path("Home/Scans/B"));
    opened(&rig, &shell, &a, 3);
    rig.settle();
    let medal = |caption: &str| oni_app::app::tiles(&shell.window).into_iter().find(|t| t.caption == caption).map_or(-1, |t| t.medal);
    assert_eq!(medal("A2"), 0);

    // B asked: its read runs, its landings held in the queue no pump runs.
    shell.open_folder(Path::new(&b));
    {
        let s = ctl.state.borrow();
        assert_eq!((s.opened(), s.shown()), (Some(b.as_str()), Some(a.as_str())), "B asked, A shown");
        assert!(!s.read_done());
        assert_eq!(s.load_bar().and_then(|bar| bar.progress), Some((3, 3)), "the load bar counts A's rows");
    }
    let at = ctl.state.borrow().order().iter().position(|p| paths::name(p) == "A2.tif").expect("A2 shown");
    shell.window.global::<Library>().invoke_tile_medal_picked(at as i32, 2);
    {
        let s = ctl.state.borrow();
        assert_eq!(names(&shell), ["A1.tif", "A2.tif", "A3.tif"], "A's rows read again under A's path");
        assert_eq!(s.load_bar().and_then(|bar| bar.progress), Some((3, 3)), "the load bar still counts A");
        assert_eq!(s.shown(), Some(a.as_str()));
    }
    assert_eq!(medal("A2"), 2, "the medal lands on A's tile");

    // Released, B's first landing replaces the folder, its rows and its tiles at once.
    let start = Instant::now();
    while ctl.state.borrow().shown() == Some(a.as_str()) {
        assert!(start.elapsed() < Duration::from_secs(30), "B's read never landed");
        rig.pump();
        std::thread::sleep(Duration::from_millis(5));
    }
    {
        let s = ctl.state.borrow();
        assert_eq!((s.opened(), s.shown()), (Some(b.as_str()), Some(b.as_str())), "B asked and shown");
        assert!(s.order().iter().all(|p| paths::holds(&b, p)), "no tile of A outlives B's first landing: {:?}", s.order());
    }
    rig.until("B read whole", Duration::from_secs(30), || ctl.state.borrow().read_done());
    assert_eq!(names(&shell), ["B1.tif"]);
    assert_eq!(medal("A2"), -1, "twin: A's tile is gone with A");
    drop((ctl, shell));
    let _ = std::fs::remove_dir_all(&d.root);
}
