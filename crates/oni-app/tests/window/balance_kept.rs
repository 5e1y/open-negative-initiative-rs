//! Auto White Balance kept ahead of the press: a frame read once lands at once, alone or six together,
//! with no reading and no count on the key; an input moved reads it again. De-dust's strokes are read
//! once the tab is left, never stroke by stroke. Roll #8's frames when `ONI_RAW_DIR` holds them.

use oni_app::app::{Options, Shell};
use oni_app::fields::Field;
use oni_app::measuring::{self, READINGS};
use oni_app::{Canvas, Library, PanelState, Store as StoreGlobal};
use oni_core::{Archive, EditorStore};
use oni_settings::{PipelineSettings, sidecar};
use slint::ComponentHandle;
use slint::platform::WindowEvent;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crate::common;

/// The press lands within the interface's frame budget when every balance is kept.
const AT_ONCE: Duration = Duration::from_millis(50);
const FRAMES: usize = 6;

/// Six of roll #8's frames as hard links beside copies of their sidecars, else six graded TIFFs.
fn frames(root: &Path) -> (PathBuf, Vec<PathBuf>) {
    let folder = root.join("Roll");
    std::fs::create_dir_all(&folder).unwrap();
    let roll = oni_fixtures::folder(oni_fixtures::RAW_DIR);
    let names: Vec<String> = (6432..6432 + FRAMES).map(|n| format!("DSC_{n}.NEF")).collect();
    if names.iter().all(|n| roll.join(n).exists()) {
        for n in &names {
            std::fs::hard_link(roll.join(n), folder.join(n)).unwrap();
            let json = format!("{n}.json");
            if roll.join(&json).exists() {
                std::fs::copy(roll.join(&json), folder.join(&json)).unwrap();
            }
            // No balance pressed yet, so a landing is the press's own.
            let unbalanced = |s: &PipelineSettings| PipelineSettings { balance_filters: None, ..s.clone() };
            sidecar::update(&folder.join(n), true, unbalanced).unwrap();
        }
    } else {
        for (i, n) in ["A", "B", "C", "D", "E", "F"].iter().enumerate() {
            common::graded_tiff(&folder.join(format!("{n}.tif")), 150, 100, 0.3 + 0.1 * i as f32);
        }
    }
    let folder = PathBuf::from(oni_library::paths::canonical(&folder));
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&folder).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e != "json")).collect();
    paths.sort();
    (folder, paths)
}

struct Rigged {
    rig: common::Rig,
    shell: Shell,
    root: PathBuf,
    paths: Vec<PathBuf>,
}

fn rigged(check: &str) -> Option<Rigged> {
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => {
            println!("SKIPPED {check}: {e}");
            return None;
        }
    };
    let root = std::env::temp_dir().join(format!("oni-app-balance-kept-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, paths) = frames(&root);
    // Decodes kept on disk, as the app keeps them.
    oni_decode::cache::keep_in(root.join("Decodes"), 8 << 30);
    let rig = common::rig(std::env::temp_dir());
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let options = Options { support: root.join("Support"), caches: root.join("Caches"), picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), oni_app::render::Engine::shared(&gpu), options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(slint::PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    shell.ctl.set_watching(false);
    shell.open_folder(&folder);
    let ctl = shell.ctl.clone();
    rig.until("the folder read", Duration::from_secs(60), || ctl.state.borrow().read_done());
    Some(Rigged { rig, shell, root, paths })
}

impl Rigged {
    /// The settings a press reads: the open frame's live ones, the others' sidecars.
    fn settings(&self, path: &Path) -> PipelineSettings {
        let s = self.shell.ctl.state.borrow();
        if s.open_path() == Some(path.to_string_lossy().as_ref()) {
            return s.store().settings().clone();
        }
        sidecar::read(path, oni_decode::decodes_via_mosaic(path), true).map(|l| l.settings).unwrap_or_default()
    }

    fn kept(&self, paths: &[PathBuf]) -> bool {
        paths.iter().all(|p| measuring::kept(&p.to_string_lossy(), &self.settings(p)).is_some())
    }

    fn balanced(&self, path: &Path) -> bool {
        self.settings(path).balance_filters.is_some()
    }

    /// The press with every balance kept: landed within the call, nothing read, no count on the key.
    fn pressed_at_once(&self, paths: &[PathBuf], what: &str) -> Duration {
        let (ctl, panel) = (&self.shell.ctl, self.shell.window.global::<PanelState>());
        self.rig.until(&format!("{what}: every balance read ahead"), Duration::from_secs(120), || self.kept(paths));
        for p in paths {
            let s = self.settings(p);
            let kept = || measuring::kept(&p.to_string_lossy(), &s).unwrap().reference.expect("a negative is read on a roll");
            match &s.roll_reference {
                // A roll of one is a roll of its own at each press, as Swift measures it afresh.
                None => assert_ne!(kept().id, kept().id, "{what}: a kept roll of one gave its id twice"),
                Some(own) => assert_eq!(kept(), *own, "{what}: twin: a frame on its roll keeps its reference"),
            }
        }
        let readings = READINGS.load(Ordering::SeqCst);
        let started = Instant::now();
        ctl.act(oni_app::Act::AutoBalance);
        let took = started.elapsed();
        assert!(paths.iter().all(|p| self.balanced(p)), "{what}: the press landed within the call");
        assert!(!panel.get_balancing(), "{what}: no count on the key");
        assert_eq!(READINGS.load(Ordering::SeqCst), readings, "{what}: a kept balance was read again");
        println!("{what}: the press with every balance kept took {took:?}");
        assert!(took < AT_ONCE, "{what}: {took:?}");
        took
    }

    /// Twin: an input moved on the frames, and the press reads each again, counting on the key. On the
    /// Roll tab, read again only once left, the frames stay unread however long the hand rests.
    fn pressed_after_an_input(&self, paths: &[PathBuf], field: Field, tab: i32, what: &str) -> Duration {
        let (ctl, w) = (&self.shell.ctl, &self.shell.window);
        ctl.act(oni_app::Act::Undo);
        self.rig.pump();
        assert!(!paths.iter().any(|p| self.balanced(p)), "{what}: one undo took the press back");
        let panel = w.global::<PanelState>();
        panel.set_tab(tab);
        panel.invoke_tab_picked(tab);
        let g = w.global::<StoreGlobal>();
        let before: Vec<PipelineSettings> = paths.iter().map(|p| self.settings(p)).collect();
        g.invoke_press(field.index(), false);
        g.invoke_drag(field.index(), 40.0, 300.0, false);
        g.invoke_release(field.index());
        self.rig.until(&format!("{what}: the input on every frame"), Duration::from_secs(10), || paths.iter().zip(&before).all(|(p, b)| self.settings(p) != *b));
        if tab == 0 {
            rested(&self.rig);
        }
        let readings = READINGS.load(Ordering::SeqCst);
        let started = Instant::now();
        ctl.act(oni_app::Act::AutoBalance);
        assert!(panel.get_balancing(), "{what}: a moved input is read again, the key counting");
        self.rig.until(&format!("{what}: the press read again"), Duration::from_secs(120), || paths.iter().all(|p| self.balanced(p)));
        let took = started.elapsed();
        let read = READINGS.load(Ordering::SeqCst) - readings;
        println!("{what}: the press reading {read} frame(s) again took {took:?}");
        assert_eq!(read, paths.len(), "{what}: each frame read again");
        ctl.act(oni_app::Act::Undo);
        self.rig.pump();
        panel.set_tab(2);
        panel.invoke_tab_picked(2);
        took
    }
}

/// The hand at rest past the deferred write, so whatever a rest asks has been asked.
fn rested(rig: &common::Rig) {
    let started = Instant::now();
    while started.elapsed() < oni_app::app::WRITE_DELAY * 3 {
        rig.pump();
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub(crate) fn a_kept_balance_lands_at_once_alone_and_six_together() {
    let check = "balance_kept::at_once";
    let Some(r) = rigged(check) else { return };
    let (ctl, w) = (r.shell.ctl.clone(), &r.shell.window);
    let lib = w.global::<Library>();
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_opened(0);
    r.rig.until("the first render", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1);
    let open = vec![PathBuf::from(ctl.state.borrow().open_path().unwrap())];
    r.pressed_at_once(&open, "the open frame");
    r.pressed_after_an_input(&open, Field::GainRed, 2, "the open frame");

    ctl.act(oni_app::Act::ToggleMode);
    r.rig.pump();
    for i in 0..FRAMES as i32 {
        lib.invoke_tile_clicked(i, i > 0, false);
    }
    r.rig.pump();
    lib.invoke_tile_opened(0);
    let canvas = w.global::<Canvas>();
    r.rig.until("six frames together", Duration::from_secs(120), || canvas.get_status().as_str() == format!("{FRAMES} frames together"));
    r.pressed_at_once(&r.paths, "six frames together");
    r.pressed_after_an_input(&r.paths, Field::ColorDensity, 0, "six frames together");
    drop(r.shell);
    let _ = std::fs::remove_dir_all(&r.root);
}

pub(crate) fn de_dust_is_read_once_its_tab_is_left_never_stroke_by_stroke() {
    let check = "balance_kept::de_dust";
    let Some(r) = rigged(check) else { return };
    let (ctl, w) = (r.shell.ctl.clone(), &r.shell.window);
    let (lib, c) = (w.global::<Library>(), w.global::<Canvas>());
    lib.invoke_tile_clicked(0, false, false);
    lib.invoke_tile_opened(0);
    r.rig.until("the first render", Duration::from_secs(120), || ctl.state.borrow().renders() >= 1 && c.get_frame().size().width > 0);
    let open = vec![PathBuf::from(ctl.state.borrow().open_path().unwrap())];
    r.rig.until("the balance read ahead", Duration::from_secs(120), || r.kept(&open));
    let rested = || rested(&r.rig);
    rested();
    let readings = READINGS.load(Ordering::SeqCst);

    ctl.act(oni_app::Act::Correction);
    let corrections = || r.settings(&open[0]).corrections.len();
    let (fx, fy, fw, fh) = (c.get_frame_x(), c.get_frame_y(), c.get_frame_width(), c.get_frame_height());
    for k in 0..2 {
        let y = fy + fh * (0.3 + 0.2 * k as f32);
        c.invoke_paint_start(fx + fw * 0.4, y);
        c.invoke_paint_move(fx + fw * 0.5, y);
        c.invoke_paint_end();
        r.rig.until("the stroke", Duration::from_secs(10), || corrections() == k + 1);
        rested();
    }
    r.rig.until("the patches", Duration::from_secs(60), || {
        let caches = oni_app::patches::caches();
        r.settings(&open[0]).corrections.iter().all(|c| oni_app::patches::file(&caches, c.id).exists())
    });
    rested();
    assert_eq!(READINGS.load(Ordering::SeqCst), readings, "a stroke was read before the tab was left");

    let panel = w.global::<PanelState>();
    panel.set_tab(2);
    panel.invoke_tab_picked(2);
    r.rig.until("the balance read on leaving", Duration::from_secs(60), || READINGS.load(Ordering::SeqCst) > readings);
    rested();
    assert_eq!(READINGS.load(Ordering::SeqCst), readings + 1, "the tab left reads the balance once");
    assert!(r.kept(&open), "the strokes' balance is kept");

    // Twin: in and out with no stroke reads nothing.
    ctl.act(oni_app::Act::Correction);
    r.rig.pump();
    panel.set_tab(2);
    panel.invoke_tab_picked(2);
    rested();
    assert_eq!(READINGS.load(Ordering::SeqCst), readings + 1, "a tab left unchanged read the balance again");
    drop(r.shell);
    let _ = std::fs::remove_dir_all(&r.root);
}
