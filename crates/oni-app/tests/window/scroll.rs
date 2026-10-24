//! Dragging is kept for the sliders, the image and the scroll knob: content scrolls by the wheel alone.

use oni_app::ScrollProbe;
use slint::platform::WindowEvent;
use slint::{ComponentHandle, PhysicalSize};
use std::path::PathBuf;

use crate::common;

pub(crate) fn a_surface_scrolls_by_the_wheel_and_never_by_a_drag() {
    use slint::platform::PointerEventButton;
    use slint::LogicalPosition;
    let rig = common::rig(PathBuf::from(env!("CARGO_TARGET_TMPDIR")));
    let probe = ScrollProbe::new().unwrap();
    rig.window.set_size(PhysicalSize::new(1440, 900));
    probe.show().unwrap();
    rig.settle();
    // A press on the page's lines, away from every control, then a long upward drag.
    let start = LogicalPosition::new(200.0, 600.0);
    rig.window.dispatch_event(WindowEvent::PointerPressed { position: start, button: PointerEventButton::Left });
    for k in 1..=20 {
        rig.pump();
        std::thread::sleep(std::time::Duration::from_millis(16));
        let at = LogicalPosition::new(200.0, 600.0 - 25.0 * k as f32);
        rig.window.dispatch_event(WindowEvent::PointerMoved { position: at });
    }
    rig.window.dispatch_event(WindowEvent::PointerReleased { position: LogicalPosition::new(200.0, 100.0), button: PointerEventButton::Left });
    rig.settle();
    assert_eq!(probe.get_scroll(), 0.0, "a drag moved the page");

    // Twin: the wheel at the same place does scroll, so the drag reached a scrollable page.
    rig.window.dispatch_event(WindowEvent::PointerScrolled { position: start, delta_x: 0.0, delta_y: -300.0 });
    rig.settle();
    assert!(probe.get_scroll() < 0.0, "the wheel did not scroll the page");
}

/// Pumps for `span`, as a hand resting.
fn wait(rig: &common::Rig, span: std::time::Duration) {
    let start = std::time::Instant::now();
    while start.elapsed() < span {
        rig.pump();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// AppKit's overlay knob: on a surface that overflows it lights while it scrolls, a moment after, and
/// while the pointer is on the edge; a surface that fits has none, and a vertical one never slides.
pub(crate) fn the_knob_shows_only_where_the_content_overflows_and_only_while_it_moves() {
    use slint::LogicalPosition;
    use std::time::Duration;
    let rig = common::rig(PathBuf::from(env!("CARGO_TARGET_TMPDIR")));
    let probe = ScrollProbe::new().unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(PhysicalSize::new(1440, 900));
    probe.show().unwrap();
    rig.settle();
    assert!(probe.get_page_overflows(), "the probe's page is taller than the window");
    assert!(!probe.get_page_lit(), "the knob shows at rest");
    assert!(!probe.get_short_overflows(), "a scroller whose content fits counts as overflowing");
    let short = LogicalPosition::new(probe.get_short_x(), probe.get_short_y());
    assert!(short.y > 0.0 && short.y < 900.0, "the short scroller is off the window");

    // The wheel over the short scroller: it cannot move, so the page under it does.
    rig.window.dispatch_event(WindowEvent::PointerScrolled { position: short, delta_x: 0.0, delta_y: -120.0 });
    wait(&rig, Duration::from_millis(100));
    assert!(probe.get_scroll() < 0.0, "the wheel did not reach the page");
    assert!(probe.get_page_lit(), "the page's knob stays dark while it scrolls");
    // Twin: the same wheel event lit nothing on the surface that fits.
    assert!(!probe.get_short_lit(), "a scroller whose content fits shows a knob");

    // A moment after the last scroll, with the pointer away from the edge, the knob goes out.
    wait(&rig, Duration::from_millis(1000));
    assert!(!probe.get_page_lit(), "the knob outlasts its linger");

    // On the edge strip it lights; back over the content it goes out again.
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(1436.0, 450.0) });
    rig.pump();
    assert!(probe.get_page_lit(), "the pointer on the edge does not light the knob");
    wait(&rig, Duration::from_millis(300));
    println!("shot {}", rig.shot("scroll-knob").display());
    // Twin: on the edge of the scroller whose content fits, nothing lights.
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(probe.get_short_right() - 4.0, probe.get_short_y()) });
    rig.pump();
    assert!(!probe.get_short_lit(), "the edge of a scroller whose content fits lights a knob");
    rig.window.dispatch_event(WindowEvent::PointerMoved { position: LogicalPosition::new(700.0, 450.0) });
    rig.pump();
    assert!(!probe.get_page_lit(), "the pointer over the content lights the knob");

    // A diagonal trackpad swipe moves the vertical page down only.
    let before = probe.get_scroll();
    rig.window.dispatch_event(WindowEvent::PointerScrolled { position: LogicalPosition::new(700.0, 450.0), delta_x: -200.0, delta_y: -60.0 });
    rig.pump();
    assert_eq!(probe.get_page_x(), 0.0, "a vertical page slid sideways");
    // Twin: the swipe did reach the page.
    assert!(probe.get_scroll() < before, "the swipe did not reach the page");
}

/// A left press at `from`, `steps` moves a frame apart to `to`, and the release there.
fn drag(rig: &common::Rig, from: (f32, f32), to: (f32, f32), steps: usize) {
    use slint::platform::PointerEventButton;
    use slint::LogicalPosition;
    let at = |t: f32| LogicalPosition::new(from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
    rig.window.dispatch_event(WindowEvent::PointerPressed { position: at(0.0), button: PointerEventButton::Left });
    for k in 1..=steps {
        rig.pump();
        rig.window.dispatch_event(WindowEvent::PointerMoved { position: at(k as f32 / steps as f32) });
    }
    rig.window.dispatch_event(WindowEvent::PointerReleased { position: at(1.0), button: PointerEventButton::Left });
    rig.pump();
}

/// A click: a press and its release at once, too short for a held press to repeat.
fn click(rig: &common::Rig, at: (f32, f32)) {
    drag(rig, at, at, 0);
}

/// The knob along a track of `page` over content `whole`: its travel, its length, and the content
/// one point of knob travel stands for, as `ScrollKnob` lays it.
fn knob(page: f32, whole: f32) -> (f32, f32, f32) {
    let room = page - 2.0 * INSET;
    let span = (room * page / whole).max(24.0).min(room);
    (room - span, span, (whole - page) / (room - span))
}

/// `ScrollStyle.inset`, `knob`, `reach` and `page-keep`.
const INSET: f32 = 3.0;
const REACH: f32 = 17.0;
const KNOB: f32 = 6.0;
const KEEP: f32 = 10.0;

/// AppKit's overlay scroller takes the pointer: the knob dragged moves the content by its own travel
/// scaled to the content's, a press on the track pages toward it and repeats while held, Option
/// jumps to the spot; the content itself never drags, and the wheel still scrolls over the knob.
pub(crate) fn the_knob_drags_the_content_and_its_track_pages_as_appkit_s() {
    use std::time::Duration;
    let rig = common::rig(PathBuf::from(env!("CARGO_TARGET_TMPDIR")));
    let probe = ScrollProbe::new().unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    // Short, so a page is a fraction of the content and two of them stop short of its end.
    rig.window.set_size(PhysicalSize::new(1440, 450));
    probe.show().unwrap();
    rig.settle();
    let (page, whole) = (probe.get_page_shown(), probe.get_page_whole());
    let (travel, span, ratio) = knob(page, whole);
    let step = (page - KEEP) / ratio;
    assert!(whole - page > 2.0 * (page - KEEP) && travel > 200.0, "the page scrolls far enough to measure: {page} of {whole}");
    let edge = 1440.0 - INSET - KNOB / 2.0;
    let offset = || -probe.get_scroll();

    // The knob held near its top and dragged 120 points: the content follows by 120 × its ratio.
    drag(&rig, (edge, INSET + 4.0), (edge, INSET + 124.0), 12);
    assert!((offset() - 120.0 * ratio).abs() < 1.0, "the knob moved the content {} for {}", offset(), 120.0 * ratio);
    // Twin: a drag that moved the content point for point, as a grab of the content would, misses.
    assert!((offset() - 120.0).abs() > 10.0, "the knob dragged the content one for one");
    assert!(probe.get_page_lit(), "the knob is dark while it is dragged");

    // Dragged past the end, it stops there; back past the start, it stops at zero.
    let at = INSET + offset() / ratio;
    drag(&rig, (edge, at + 4.0), (edge, at + 4.0 + 2.0 * page), 8);
    assert!((offset() - (whole - page)).abs() < 0.5, "a drag past the end left {}", offset());
    drag(&rig, (edge, INSET + travel + 4.0), (edge, -page), 8);
    assert_eq!(offset(), 0.0, "a drag past the start left the content off zero");

    // A click on the track under the knob jumps a page less `page-keep`; one above it jumps back.
    click(&rig, (edge, page - 20.0));
    assert!((offset() - (page - KEEP)).abs() < 0.5, "a click under the knob paged to {}", offset());
    click(&rig, (edge, INSET + 1.0));
    assert_eq!(offset(), 0.0, "a click above the knob did not page back");
    // Twin: the same click over the content beside the strip pages nothing.
    click(&rig, (1440.0 - REACH - 4.0, page - 20.0));
    assert_eq!(offset(), 0.0, "a click on the content paged it");

    // Held on the track, it pages again until the knob stands under the pointer, and no further.
    let target = INSET + 2.0 * step + 0.7 * span;
    assert!(target > INSET + step + span, "the knob covers the pointer after one page already");
    let held = slint::LogicalPosition::new(edge, target);
    rig.window.dispatch_event(WindowEvent::PointerPressed { position: held, button: slint::platform::PointerEventButton::Left });
    wait(&rig, Duration::from_millis(1500));
    // Twin of the stop: two pages, where one would mean no repeat and three the knob past the pointer.
    assert!((offset() - 2.0 * (page - KEEP)).abs() < 0.5, "the held press paged to {}", offset());
    rig.window.dispatch_event(WindowEvent::PointerReleased { position: held, button: slint::platform::PointerEventButton::Left });
    rig.pump();

    // Option-click jumps the knob's middle to the spot.
    let spot = INSET + span / 2.0 + travel * 0.25;
    rig.window.dispatch_event(WindowEvent::KeyPressed { text: slint::platform::Key::Alt.into() });
    click(&rig, (edge, spot));
    rig.window.dispatch_event(WindowEvent::KeyReleased { text: slint::platform::Key::Alt.into() });
    assert!((offset() - 0.25 * (whole - page)).abs() < 1.0, "Option-click jumped to {}", offset());

    // The wheel over the knob's strip still scrolls; a drag of the content never does.
    let before = offset();
    rig.window.dispatch_event(WindowEvent::PointerScrolled { position: slint::LogicalPosition::new(edge, page / 2.0), delta_x: 0.0, delta_y: -100.0 });
    rig.pump();
    assert!(offset() > before, "the wheel over the strip did not scroll");
    let before = offset();
    drag(&rig, (200.0, 400.0), (200.0, 100.0), 10);
    assert_eq!(offset(), before, "a drag of the content scrolled it");
}

/// The gallery's knob over the document Rust scrolls: a drag takes the wheel's way, so the tiles in
/// view, the pinned heading and the thumbnails follow it, and a move over a long roll stays light.
pub(crate) fn the_gallery_knob_scrolls_the_grid_the_wheel_s_way() {
    use oni_app::app::{Options, Shell};
    use oni_app::Library;
    use oni_core::{Archive, EditorStore};
    use slint::Model;
    use std::time::{Duration, Instant};
    let gpu = match oni_app::render::Device::open() {
        Ok(gpu) => gpu,
        Err(e) => return eprintln!("{e}: the gallery knob is not exercised"),
    };
    let root = std::env::temp_dir().join(format!("oni-app-gallery-knob-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let (folder, support, caches) = (root.join("Scans"), root.join("Support"), root.join("Caches"));
    for i in 0..1200 {
        common::plain_tiff(&folder.join(format!("P{i:04}.tif")), 24, 16, 0.3 + (i % 40) as f32 / 80.0);
    }
    let rig = common::rig(PathBuf::from(env!("CARGO_TARGET_TMPDIR")));
    let store = EditorStore::new(Archive::new(Vec::new(), oni_decode::decodes_via_mosaic));
    let engine = oni_app::render::Engine::shared(&gpu);
    let options = Options { support, caches, picker: Box::new(|| None), ..Options::default() };
    let shell = Shell::new(Box::new(store), engine, options).unwrap();
    rig.window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 1.0 });
    rig.window.set_size(PhysicalSize::new(1280, 800));
    shell.window.show().unwrap();
    let ctl = shell.ctl.clone();
    let lib = shell.window.global::<Library>();
    shell.open_folder(&folder);
    rig.until("the folder read", Duration::from_secs(60), || ctl.state.borrow().read_done());
    rig.settle();
    let (width, page) = ctl.state.borrow().grid();
    let whole = lib.get_content_height();
    let (travel, _, ratio) = knob(page, whole);
    assert!(lib.get_scroll() == 0.0 && whole > 20.0 * page, "a long roll opens at its top: {whole} for {page}");
    assert!(ctl.state.borrow().visible().start == 0 && !lib.get_pinned_shown(), "at the top the first tiles show, no heading pinned");
    let (left, top) = (lib.get_grid_x(), lib.get_grid_y());
    let edge = left + width - INSET - KNOB / 2.0;

    // A drag of 300 points in 60 moves, each timed from the event to the tiles laid.
    let mut slowest = Duration::ZERO;
    rig.window.dispatch_event(WindowEvent::PointerPressed { position: slint::LogicalPosition::new(edge, top + INSET + 2.0), button: slint::platform::PointerEventButton::Left });
    for k in 1..=60 {
        let at = slint::LogicalPosition::new(edge, top + INSET + 2.0 + 5.0 * k as f32);
        let start = Instant::now();
        rig.window.dispatch_event(WindowEvent::PointerMoved { position: at });
        slowest = slowest.max(start.elapsed());
        rig.pump();
    }
    rig.window.dispatch_event(WindowEvent::PointerReleased { position: slint::LogicalPosition::new(edge, top + 302.0), button: slint::platform::PointerEventButton::Left });
    rig.pump();
    println!("slowest knob move over {} photos: {:.2} ms", ctl.state.borrow().photos(), slowest.as_secs_f64() * 1e3);
    let scroll = lib.get_scroll();
    assert!((scroll - 300.0 * ratio).abs() < 1.0, "the knob scrolled the grid {scroll} for {}", 300.0 * ratio);
    assert!(travel > 300.0 && scroll > 10.0 * page, "the drag crossed many pages: {scroll}");
    assert!(slowest < Duration::from_millis(16), "a knob move took {slowest:?}, past a frame");

    // The tiles laid are those in view, the heading pinned at the new top, as after a wheel.
    let tiles = oni_app::app::tiles(&shell.window);
    assert!(ctl.state.borrow().visible().start > 0, "the visible range stayed at the top");
    assert!(tiles.iter().any(|t| t.y <= scroll + page / 2.0 && t.y + t.height >= scroll), "no tile laid where the grid now shows");
    assert!(tiles.iter().all(|t| t.y + t.height > scroll - page && t.y < scroll + 2.0 * page), "tiles laid far from the view");
    assert!(lib.get_pinned_shown() && lib.get_pinned_y() == scroll, "the heading is not pinned at the new top");
    // The cells under the eye ask their thumbnails: each tile in view gets its picture.
    rig.until("the thumbnails in view", Duration::from_secs(60), || {
        lib.get_tiles().iter().filter(|t| t.y + t.height > lib.get_scroll() && t.y < lib.get_scroll() + page).all(|t| t.picture.size().width > 0)
    });

    // A click on the track under the knob pages the grid by a page less `page-keep`.
    click(&rig, (edge, top + page - 10.0));
    assert!((lib.get_scroll() - (scroll + page - KEEP)).abs() < 0.5, "the track click paged to {}", lib.get_scroll());
    // Twin: a drag on the tiles scrolls nothing.
    let before = lib.get_scroll();
    drag(&rig, (left + width / 2.0, top + page - 40.0), (left + width / 2.0, top + 40.0), 10);
    assert_eq!(lib.get_scroll(), before, "a drag on the tiles scrolled the grid");
    drop(shell);
    let _ = std::fs::remove_dir_all(&root);
}
