//! The whole window on Slint's software renderer, no screen: roll #8 opened from a mirror of hard
//! links, so no sidecar is written beside the originals; screenshots go to `ONI_SHOTS`.

use oni_app::app::{self, Options, Shell};
use oni_app::fields::Field;
use oni_app::{App, Bar, Library, MainWindow, PanelState, Store as StoreGlobal};
use oni_core::{Archive, EditorStore};
use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType, Rgb565Pixel};
use slint::platform::{EventLoopProxy, Platform, WindowAdapter, WindowEvent};
use slint::{ComponentHandle, PhysicalSize};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

type Queued = Box<dyn FnOnce() + Send>;

struct Headless {
    window: Rc<MinimalSoftwareWindow>,
    queue: Arc<Mutex<Vec<Queued>>>,
    start: Instant,
}

struct Proxy(Arc<Mutex<Vec<Queued>>>);

impl EventLoopProxy for Proxy {
    fn quit_event_loop(&self) -> Result<(), slint::EventLoopError> {
        Ok(())
    }

    fn invoke_from_event_loop(&self, event: Queued) -> Result<(), slint::EventLoopError> {
        self.0.lock().unwrap().push(event);
        Ok(())
    }
}

impl Platform for Headless {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.window.clone())
    }

    fn duration_since_start(&self) -> Duration {
        self.start.elapsed()
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn EventLoopProxy>> {
        Some(Box::new(Proxy(self.queue.clone())))
    }
}

struct Rig {
    window: Rc<MinimalSoftwareWindow>,
    queue: Arc<Mutex<Vec<Queued>>>,
    shots: PathBuf,
}

impl Rig {
    /// Runs what background threads posted, then the timers and animations.
    fn pump(&self) {
        let jobs: Vec<Queued> = std::mem::take(&mut *self.queue.lock().unwrap());
        for job in jobs {
            job();
        }
        slint::platform::update_timers_and_animations();
    }

    fn until(&self, what: &str, limit: Duration, done: impl Fn() -> bool) {
        let start = Instant::now();
        while !done() {
            assert!(start.elapsed() < limit, "timed out waiting for {what}");
            self.pump();
            std::thread::sleep(Duration::from_millis(20));
        }
        self.pump();
        println!("{what}: {:.1} s", start.elapsed().as_secs_f64());
    }

    /// Lets springs and the tooltip delay finish, as a person waits before looking.
    fn settle(&self) {
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(900) {
            self.pump();
            std::thread::sleep(Duration::from_millis(16));
        }
    }

    fn shot(&self, name: &str) -> PathBuf {
        self.window.request_redraw();
        let size = self.window.size();
        let (w, h) = (size.width as usize, size.height as usize);
        let mut buffer = vec![Rgb565Pixel(0); w * h];
        self.window.draw_if_needed(|r| {
            r.render(&mut buffer, w);
        });
        let path = self.shots.join(format!("{name}.png"));
        let file = std::fs::File::create(&path).unwrap();
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        let bytes: Vec<u8> = buffer
            .iter()
            .flat_map(|p| {
                let v = p.0;
                let (r, g, b) = ((v >> 11) & 0x1f, (v >> 5) & 0x3f, v & 0x1f);
                [(r * 255 / 31) as u8, (g * 255 / 63) as u8, (b * 255 / 31) as u8]
            })
            .collect();
        writer.write_image_data(&bytes).unwrap();
        path
    }

    fn key(&self, text: &str) {
        let t: slint::SharedString = text.into();
        self.window.dispatch_event(WindowEvent::KeyPressed { text: t.clone() });
        self.window.dispatch_event(WindowEvent::KeyReleased { text: t });
        self.pump();
    }

    fn chord(&self, modifier: slint::platform::Key, text: &str) {
        let m: slint::SharedString = modifier.into();
        self.window.dispatch_event(WindowEvent::KeyPressed { text: m.clone() });
        self.key(text);
        self.window.dispatch_event(WindowEvent::KeyReleased { text: m });
        self.pump();
    }
}

/// Roll #8 as hard links beside copies of its sidecars and roll marker, in a fresh folder.
fn mirror(roll: &Path) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oni-app-headless-{}", std::process::id())).join("Roll #8");
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    std::fs::create_dir_all(&dir).unwrap();
    for entry in std::fs::read_dir(roll).unwrap() {
        let from = entry.unwrap().path();
        let to = dir.join(from.file_name().unwrap());
        let name = from.file_name().unwrap().to_string_lossy().to_string();
        if name.ends_with(".json") {
            std::fs::copy(&from, &to).unwrap();
        } else {
            std::fs::hard_link(&from, &to).unwrap();
        }
    }
    dir
}

/// Each file's name, size and modification time, in name order.
fn stamps(dir: &Path) -> Vec<(String, u64, std::time::SystemTime)> {
    let mut out: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| {
            let e = e.unwrap();
            let m = e.metadata().unwrap();
            (e.file_name().to_string_lossy().into_owned(), m.len(), m.modified().unwrap())
        })
        .collect();
    out.sort();
    out
}

fn label_of(path: &str) -> oni_settings::Medal {
    oni_settings::sidecar::read(Path::new(path), true, true).unwrap().label
}

#[test]
fn the_window_opens_roll_8_and_a_slider_drives_the_canvas() {
    let roll = oni_fixtures::folder(oni_fixtures::RAW_DIR);
    if !roll.join("DSC_6440.NEF").exists() {
        eprintln!("roll #8 is absent: the window is not exercised");
        return;
    }
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the window is not exercised"),
    };
    let shots = std::env::var_os("ONI_SHOTS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    std::fs::create_dir_all(&shots).unwrap();
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    let queue: Arc<Mutex<Vec<Queued>>> = Arc::default();
    slint::platform::set_platform(Box::new(Headless { window: window.clone(), queue: queue.clone(), start: Instant::now() })).unwrap();
    let rig = Rig { window: window.clone(), queue, shots };

    let originals = stamps(&roll);
    let folder = mirror(&roll);
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    println!("rendering on {}", engine.lock().unwrap().adapter());
    let shell = Shell::new(Box::new(store), engine, Options { scale: 2.0, ..Default::default() }).unwrap();
    window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 2.0 });
    window.set_size(PhysicalSize::new(2 * 1280, 2 * 800));
    shell.window.show().unwrap();
    shell.open_folder(&folder);
    let ctl = shell.ctl.clone();
    let w: &MainWindow = &shell.window;

    // The editor on the first frame, once its negative is decoded and rendered.
    rig.until("the first render", Duration::from_secs(300), || ctl.state.borrow().renders() >= 1);
    rig.settle();
    let first = ctl.state.borrow().open_path().unwrap().to_string();
    assert!(first.ends_with("DSC_6432.NEF"), "{first}");
    let canvas = w.global::<oni_app::Canvas>();
    assert!(canvas.get_status().contains(" × "), "{}", canvas.get_status());
    println!("shot {}", rig.shot("editor-color").display());

    // A drag on Exposure: notches at the hand's pace, one store step, one render at least.
    let g = w.global::<StoreGlobal>();
    let field = Field::Exposure.index();
    let before = ctl.state.borrow().store().get(Field::Exposure);
    let renders = ctl.state.borrow().renders();
    let span = 300.0;
    assert!(!g.invoke_press(field, false), "a first press is no double click");
    for k in 1..=12 {
        g.invoke_drag(field, k as f32 * 5.0, span, false);
        rig.pump();
        std::thread::sleep(Duration::from_millis(10));
    }
    g.invoke_release(field);
    rig.pump();
    let (low, high) = Field::Exposure.spec().range;
    let expected = before + 60.0 / span * (high - low);
    let after = ctl.state.borrow().store().get(Field::Exposure);
    assert!((after - expected).abs() < 1e-3, "{after} against {expected}");
    assert!((app::fraction(&g, Field::Exposure) - (expected - low) / (high - low)).abs() < 1e-4);
    assert_eq!(g.get_exposure_readout(), Field::Exposure.spec().readout.format(after));
    rig.until("the dragged render", Duration::from_secs(120), || ctl.state.borrow().renders() > renders);
    assert!(w.global::<Bar>().get_can_undo());
    assert!(g.get_lighting_modified() && !g.get_effects_modified());
    rig.settle();
    println!("shot {}", rig.shot("editor-color-dragged").display());

    // One undo takes the whole drag back, and the knob springs home.
    rig.chord(slint::platform::Key::Control, "z");
    assert!((ctl.state.borrow().store().get(Field::Exposure) - before).abs() < 1e-6, "the drag was one step");
    rig.settle();
    assert!((app::fraction(&g, Field::Exposure) - (before - low) / (high - low)).abs() < 1e-4);
    // Twin: the press of a double click resets to the rest instead of starting a drag.
    g.invoke_press(field, false);
    g.invoke_release(field);
    assert!(g.invoke_press(field, false), "the second press within the interval is a double click");
    g.invoke_drag(field, 100.0, span, false);
    g.invoke_release(field);
    rig.pump();
    assert_eq!(ctl.state.borrow().store().get(Field::Exposure), Field::Exposure.spec().base);
    rig.chord(slint::platform::Key::Control, "z");
    rig.settle();
    // One more notch alone, from the store's write to the canvas: the render a held slider waits on.
    std::thread::sleep(Duration::from_millis(600));
    g.invoke_press(field, false);
    g.invoke_drag(field, 5.0, span, false);
    g.invoke_release(field);
    let revision = ctl.state.borrow().store().revision();
    rig.until("one notch", Duration::from_secs(60), || ctl.state.borrow().rendered() >= revision);
    rig.chord(slint::platform::Key::Control, "z");

    // The Roll tab, then the bar's other face, the gallery, once every thumbnail is in.
    w.global::<PanelState>().set_tab(0);
    rig.settle();
    println!("shot {}", rig.shot("editor-roll").display());
    w.global::<PanelState>().set_tab(2);

    rig.chord(slint::platform::Key::Control, "g");
    assert!(w.global::<App>().get_gallery(), "⌘G opens the gallery");
    rig.until("ten thumbnails", Duration::from_secs(600), || ctl.state.borrow().loaded() >= 10);
    let tiles = app::tiles(w);
    assert_eq!(tiles.len(), 10, "every cell of a ten-photo roll is in view");
    // Both faces through the pipeline: every tile drawn, the Scanner face's among them.
    let mut scanner_drawn = 0;
    for t in &tiles {
        let path = folder.join(format!("{}.NEF", t.caption));
        let face = oni_settings::sidecar::read(&path, true, true).unwrap().settings.face;
        assert!(t.picture.size().width > 0, "{}: {face:?} left empty", t.caption);
        scanner_drawn += usize::from(face == oni_settings::Face::Scanner);
    }
    println!("10 of 10 thumbnails drawn, {scanner_drawn} on the Scanner face");
    // Twin: the roll holds Scanner-face frames, so a renderer refusing that face fails above.
    assert!(scanner_drawn > 0);
    w.global::<Library>().invoke_tile_clicked(3, false, false);
    rig.pump();
    rig.settle();
    println!("shot {}", rig.shot("gallery").display());

    // A bare medal key labels the selection; typed into the description field, it stays a character.
    let fourth = folder.join("DSC_6444.NEF").to_string_lossy().into_owned();
    assert_ne!(label_of(&fourth), oni_settings::Medal::Silver);
    rig.key("2");
    assert_eq!(label_of(&fourth), oni_settings::Medal::Silver);
    rig.key("\t");
    assert!(!w.get_keys_focused(), "Tab moves the keyboard into the description field");
    rig.key("1");
    assert_eq!(label_of(&fourth), oni_settings::Medal::Silver, "a key typed into a field never labels");
    assert_eq!(w.global::<Library>().get_description(), "1");
    // Twin: the same key with the key scope focused does label.
    w.invoke_refocus();
    rig.key("1");
    assert_eq!(label_of(&fourth), oni_settings::Medal::Bronze);
    rig.key("3");
    assert_eq!(label_of(&fourth), oni_settings::Medal::Gold);
    rig.settle();
    println!("shot {}", rig.shot("gallery-labelled").display());

    // A photo's row in the library list opens it and selects its tile, with no borrow left held.
    let rows = w.global::<Library>().get_rows();
    let row = (0..slint::Model::row_count(&rows))
        .find(|&i| slint::Model::row_data(&rows, i).is_some_and(|r| r.kind == 2 && r.title.contains("DSC_6440")))
        .expect("the list holds DSC_6440's row");
    w.global::<Library>().invoke_row_clicked(row as i32);
    rig.pump();
    assert!(ctl.state.borrow().open_path().unwrap().ends_with("DSC_6440.NEF"));
    let selected: Vec<i32> = app::tiles(w).iter().filter(|t| t.selected).map(|t| t.index).collect();
    assert_eq!(selected.len(), 1, "{selected:?}");
    assert!(app::tiles(w)[selected[0] as usize].caption.contains("DSC_6440"));
    // Twin: a tile click then moves the selection elsewhere, the list's row no longer its target.
    w.global::<Library>().invoke_tile_clicked(5, false, false);
    rig.pump();
    let selected: Vec<i32> = app::tiles(w).iter().filter(|t| t.selected).map(|t| t.index).collect();
    assert_eq!(selected, vec![5]);

    // A Scanner-face frame opened in the editor renders on the canvas, through the Frontier's table.
    let scanner = tiles
        .iter()
        .find(|t| oni_settings::sidecar::read(&folder.join(format!("{}.NEF", t.caption)), true, true).unwrap().settings.face == oni_settings::Face::Scanner)
        .map(|t| (t.index, t.caption.to_string()))
        .expect("roll #8 holds a Scanner-face frame");
    let renders = ctl.state.borrow().renders();
    w.global::<Library>().invoke_tile_opened(scanner.0);
    rig.until("the Scanner-face render", Duration::from_secs(300), || ctl.state.borrow().renders() > renders);
    let status = canvas.get_status();
    assert!(status.starts_with(&scanner.1) && !status.contains("Not rendered"), "{status}");
    assert!(canvas.get_frame().size().width > 0, "the canvas shows the Scanner-face frame");
    rig.settle();
    println!("shot {}", rig.shot("editor-scanner-face").display());

    // Every write went to the mirror: the originals' folder reads as it did before the run.
    assert_eq!(stamps(&roll), originals);
    assert_ne!(stamps(&folder), stamps(&roll), "twin: the mirror's sidecars were written");
    let _ = std::fs::remove_dir_all(folder.parent().unwrap());
}
