//! What a launch shows: the app's window opens as the starter, each step on its band while it runs,
//! then grows into the app; the recalibration notice hung in the window. Shot to `ONI_SHOTS`.

use oni_app::render::{Device, Engine, Kernels};
use oni_app::{MainWindow, Notice, Starter, starter};
use slint::platform::WindowEvent;
use slint::{ComponentHandle, PhysicalSize};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::common;

const STEPS: [&str; 3] = ["Reading the library", "Building the app", "Opening the last folder"];

fn physical(points: f32) -> u32 {
    (points * 2.0).round() as u32
}

pub(crate) fn the_starter_says_each_step_then_grows_into_the_app_and_the_notice_covers_the_window() {
    let shots = std::env::var_os("ONI_SHOTS").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("shots"));
    let rig = common::rig(shots);
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 2.0 });

    // Twin first: kernels still compiling hold the starter past its last step.
    {
        let window = MainWindow::new().unwrap();
        oni_app::keycaps::serve(&window);
        let opened = Rc::new(Cell::new(false));
        let seen = opened.clone();
        starter::launch(&window, Instant::now(), Vec::new(), vec![starter::step(STEPS[0], || {})], Arc::new(Kernels::default()), move || seen.set(true)).unwrap();
        let band = window.global::<Starter>();
        rig.until("the step done", Duration::from_secs(5), || !band.get_done().is_empty());
        rig.settle();
        assert!(!opened.get() && band.get_shown(), "kernels under way hold the starter");
        println!("shot {}", rig.shot("starter").display());
        window.hide().unwrap();
    }

    let gpu = match Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the starter is not exercised"),
    };
    let engine = Engine::shared(&gpu);
    let kernels = Arc::new(Kernels::default());
    let (warming, counted) = (engine.clone(), kernels.clone());
    std::thread::spawn(move || warming.prewarm_counted(&counted));
    let window = MainWindow::new().unwrap();
    oni_app::keycaps::serve(&window);
    let said = Rc::new(RefCell::new(Vec::<String>::new()));
    let steps = STEPS
        .map(|words| {
            let (said, weak) = (said.clone(), window.as_weak());
            starter::step(words, move || said.borrow_mut().push(weak.upgrade().unwrap().global::<Starter>().get_stage().to_string()))
        })
        .into();
    let opened = Rc::new(Cell::new(0));
    let seen = opened.clone();
    let before = vec![("Starting the graphics card", Duration::from_millis(1200))];
    starter::launch(&window, Instant::now(), before, steps, kernels.clone(), move || seen.set(seen.get() + 1)).unwrap();
    let band = window.global::<Starter>();
    assert!(band.get_shown());
    let size = rig.window.size();
    assert_eq!((size.width, size.height), (physical(band.get_width()), physical(band.get_height())), "the window opens at the starter's size");
    rig.until("the app", Duration::from_secs(60), || opened.get() > 0);
    assert_eq!(*said.borrow(), STEPS, "each step is on the band while it runs");
    assert!(!band.get_shown() && kernels.finished());
    let size = rig.window.size();
    assert_eq!((size.width, size.height), (physical(band.get_app_width()), physical(band.get_app_height())), "the same window grows into the app");
    rig.settle();
    assert_eq!(opened.get(), 1, "the app opens once");

    rig.window.set_size(PhysicalSize::new(2 * 1440, 2 * 900));
    window.global::<Notice>().set_shown(true);
    rig.settle();
    println!("shot {}", rig.shot("notice").display());
    let closed = Rc::new(Cell::new(false));
    let seen = closed.clone();
    window.global::<Notice>().on_close(move || seen.set(true));
    let click = |x: f32, y: f32| {
        let position = slint::LogicalPosition::new(x, y);
        rig.window.dispatch_event(WindowEvent::PointerPressed { position, button: slint::platform::PointerEventButton::Left });
        rig.window.dispatch_event(WindowEvent::PointerReleased { position, button: slint::platform::PointerEventButton::Left });
        rig.pump();
    };
    // Modal as Swift's sheet: a click beside the card leaves it open.
    click(40.0, 450.0);
    assert!(!closed.get(), "a click beside the notice leaves it open");
    // GOT IT sits at the card's right, under its bullets: the sweep finds it right of the middle.
    let mut found = None;
    'sweep: for y in (24..876).step_by(4) {
        for x in (720 - 210..720 + 210).step_by(6) {
            click(x as f32, y as f32);
            if closed.get() {
                found = Some(x);
                break 'sweep;
            }
        }
    }
    let x = found.expect("GOT IT closes the notice");
    assert!(x > 720 + 60, "twin: GOT IT is not under the card's middle but at its right, x {x}");
}
