//! The archive's tree is cut again only when its shape moves: opening a photo and a thumbnail
//! landing cut nothing, a fold lays the rows again, a read that finds a new folder cuts the tree.

use oni_app::app::{self, Options, Shell};
use oni_app::Library;
use oni_core::{Archive, EditorStore};
use slint::{ComponentHandle, Model};
use std::path::PathBuf;
use std::time::Duration;

use crate::common;

fn folder_row(lib: &Library, title: &str) -> Option<usize> {
    lib.get_rows().iter().position(|r| r.kind != 2 && r.title == title)
}

pub(crate) fn opening_a_photo_or_a_thumbnail_landing_cuts_no_tree() {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the tree's cost is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-tree-cost-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (scans, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for i in 0..6 {
        common::plain_tiff(&scans.join("Roll A").join(format!("A{i:02}.tif")), 96, 64, 0.3 + i as f32 / 60.0);
        common::plain_tiff(&scans.join("Roll B").join(format!("B{i:02}.tif")), 96, 64, 0.65 + i as f32 / 100.0);
    }
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let lib = w.global::<Library>();
    shell.open_folder(&scans);
    rig.until("the folder read", Duration::from_secs(30), || ctl.state.borrow().read_done());
    rig.until("every thumbnail landed", Duration::from_secs(60), || ctl.state.borrow().loaded() >= 12);
    rig.settle();
    let cuts = oni_app::library::cuts;
    let before = cuts();

    // A photo opened in the editor: its row lit and centred, nothing cut or laid again.
    let tile = app::tiles(w).into_iter().find(|t| t.caption == "B03").expect("B03's tile");
    lib.invoke_tile_opened(tile.index);
    rig.until("a photo rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    rig.settle();
    assert_eq!(ctl.state.borrow().open_path().map(|p| p.ends_with("B03.tif")), Some(true));
    assert_eq!(cuts(), before, "opening a photo cuts no tree and lays no rows");

    // Its thumbnail drawn again once it rests, landing on its row and its tile alone.
    let landed = ctl.state.borrow().thumbnails_landed();
    rig.until("the open photo's thumbnail drawn again", Duration::from_secs(30), || ctl.state.borrow().thumbnails_landed() > landed);
    rig.settle();
    assert_eq!(cuts(), before, "a thumbnail landing cuts no tree and lays no rows");

    // A read again that finds nothing new keeps the tree held.
    let reread = |rig: &common::Rig| {
        lib.invoke_tree_reload();
        rig.until("the folder read again", Duration::from_secs(30), || ctl.state.borrow().read_done());
        rig.settle();
    };
    reread(&rig);
    assert_eq!(cuts().0, before.0, "a read that finds the same rows cuts no tree");

    // Twin: a fold lays the rows again, the tree kept.
    let roll_b = folder_row(&lib, "Roll B").expect("Roll B's row");
    lib.invoke_row_folded(roll_b as i32);
    rig.pump();
    let folded = cuts();
    assert_eq!(folded.0, before.0, "a fold cuts no tree");
    assert!(folded.1 > before.1, "twin: a fold lays the rows again: {folded:?} against {before:?}");

    // Twin: a folder added on the disk cuts the tree again, and its row shows.
    common::plain_tiff(&scans.join("Roll C").join("C00.tif"), 96, 64, 0.5);
    reread(&rig);
    rig.until("Roll C's row", Duration::from_secs(30), || folder_row(&lib, "Roll C").is_some());
    assert!(cuts().0 > before.0, "twin: a new folder cuts the tree again");
    assert!(!lib.get_rows().iter().any(|r| r.kind == 2 && r.title.starts_with("B0")), "the hand's fold stays through the cut");
    let _ = std::fs::remove_dir_all(&root);
}

/// What the interface thread spent on the jobs the other threads posted, while `done` is false.
struct Busy {
    jobs: usize,
    total: Duration,
    longest: Duration,
}

fn busy_until(rig: &common::Rig, limit: Duration, done: impl Fn() -> bool) -> Busy {
    let start = std::time::Instant::now();
    let mut busy = Busy { jobs: 0, total: Duration::ZERO, longest: Duration::ZERO };
    while !done() {
        if start.elapsed() > limit {
            println!("(timed out after {limit:?})");
            break;
        }
        let jobs: Vec<common::Queued> = std::mem::take(&mut *rig.queue.lock().unwrap());
        for job in jobs {
            let at = std::time::Instant::now();
            job();
            let took = at.elapsed();
            busy.jobs += 1;
            busy.total += took;
            busy.longest = busy.longest.max(took);
        }
        slint::platform::update_timers_and_animations();
        std::thread::sleep(Duration::from_millis(2));
    }
    busy
}

fn said(name: &str, busy: &Busy) {
    let ms = |d: Duration| d.as_secs_f64() * 1e3;
    println!("{name:<40} {:>5} jobs  {:>9.1} ms in all  {:>8.2} ms the longest", busy.jobs, ms(busy.total), ms(busy.longest));
}

fn timed(name: &str, body: impl FnOnce()) {
    let at = std::time::Instant::now();
    body();
    println!("{name:<40} {:>9.2} ms on the interface thread", at.elapsed().as_secs_f64() * 1e3);
}

/// The interface thread's share of each event on a folder of 7000 photos, printed; run on demand
/// with `ONI_TREE_BENCH=1`, as drawing every thumbnail takes a minute.
pub(crate) fn the_tree_s_cost_on_a_big_folder() {
    if std::env::var_os("ONI_TREE_BENCH").is_none() {
        return eprintln!("ONI_TREE_BENCH unset: the big folder is not laid");
    }
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the tree's cost is not measured"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-tree-bench-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (scans, support, caches) = (root.join("Library"), root.join("Support"), root.join("Caches"));
    let started = std::time::Instant::now();
    let mut total = 0;
    for f in 0..200 {
        for p in 0..25 {
            common::plain_tiff(&scans.join(format!("{}/Roll {f}/F{p:02}.tif", 2000 + f / 20)), 16, 16, total as f32 / 7000.0);
            total += 1;
        }
    }
    for p in 0..2000 {
        common::plain_tiff(&scans.join(format!("Flat/IMG_{p:04}.tif")), 16, 16, total as f32 / 7000.0);
        total += 1;
    }
    println!("{total} photos written in {:.1} s", started.elapsed().as_secs_f64());
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let rig = common::rig(shots);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let w = &shell.window;
    let lib = w.global::<Library>();
    let read = |rig: &common::Rig, name: &str| said(name, &busy_until(rig, Duration::from_secs(600), || ctl.state.borrow().read_done()));
    timed("open the folder (the call)", || shell.open_folder(&scans));
    read(&rig, "first read: landings");
    said("thumbnail pass: landings", &busy_until(&rig, Duration::from_secs(900), || ctl.state.borrow().loaded() >= total));
    rig.settle();
    timed("read again (the call)", || lib.invoke_tree_reload());
    read(&rig, "read again, nothing moved: landings");
    rig.settle();
    let tile = app::tiles(w).into_iter().next().expect("a tile");
    timed("open a photo", || lib.invoke_tile_opened(tile.index));
    rig.until("a photo rendered", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    let landed = ctl.state.borrow().thumbnails_landed();
    said("its thumbnail drawn again", &busy_until(&rig, Duration::from_secs(20), || ctl.state.borrow().thumbnails_landed() > landed));
    rig.settle();
    let roll = folder_row(&lib, "Roll 0").expect("Roll 0's row");
    timed("fold a folder", || lib.invoke_row_folded(roll as i32));
    timed("unfold it", || lib.invoke_row_folded(roll as i32));
    timed("medal the open photo", || ctl.act(oni_app::Act::MedalGold));
    timed("medal it again", || ctl.act(oni_app::Act::MedalNone));
    rig.settle();
    let _ = std::fs::remove_dir_all(&root);
}
