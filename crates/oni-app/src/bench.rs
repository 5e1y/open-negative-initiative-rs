//! `ONI_BENCH=1`: once the open frame is on the canvas, sixty notches dragged on Exposure at a
//! trackpad's pace through the slider's own callbacks, then the notch centiles, without a hand.
//! `ONI_BENCH=resize` also resizes the window on every notch, so the surface is reconfigured mid-render;
//! `ONI_BENCH=settled` waits for every thumbnail first, so the drag has the GPU to itself;
//! `ONI_BENCH=long` then drags slowly for ten seconds and reports the image's cadence;
//! `ONI_BENCH=zoom` double-clicks to 100 %, waits for full resolution, pans, and returns to fit;
//! `ONI_BENCH=compare` turns Compare on, drags Exposure and the split, and counts the befores;
//! `ONI_BENCH=crop` frames through the Crop tab's keys and the canvas's handles, then undoes it all;
//! `ONI_BENCH=dedust` paints a stroke in De-dust, waits for its patch, then removes it.
//! `ONI_BENCH=flows` walks the owner's flow in the real window: the gallery, a click that selects in
//! the gallery and the sidebar alike, a double click that opens, an edit, three frames over.
//! `ONI_BENCH=presets` saves a preset, raises the deck under the pointer, previews the card by
//! hovering it and applies it by a click, all through the window's pointer events; run it with
//! `ONI_SUPPORT` on a scratch folder, as it writes a preset.
//! `ONI_BENCH=console` opens the console window and waits for the session's log to show in it.
//! `ONI_BENCH=prepare` prepares the open frame's roll from the gallery: the steps by the panel's key,
//! the base clicked on its scan, a frame flagged on the sheet, the measure landed, then undone.
//! `ONI_BENCH=sheet` picks the gallery's first six frames and makes their contact sheet, which it
//! leaves beside them, opened.

use crate::Store as StoreGlobal;
use crate::app::Ctl;
use crate::fields::Field;
use slint::{ComponentHandle, Model, Timer, TimerMode};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

pub const NOTCHES: usize = 60;
/// Ten seconds at the trackpad's pace, a tenth of a point a notch: one way, so no turn hands the
/// same value twice, and inside the range.
const LONG: usize = 1200;
const LONG_STEP: f32 = 0.1;
/// A trackpad reports at 120 Hz.
const PACE: Duration = Duration::from_micros(8_333);
/// Two points a notch on a 300-point track: the drag crosses two fifths of it, inside the range.
const STEP: f32 = 2.0;
const SPAN: f32 = 300.0;
/// The first render waited for, decode included, before the bench gives up.
const LOADING: Duration = Duration::from_secs(600);

/// On the main screen, whose refresh the gaps are judged against.
const ORIGIN: (f32, f32) = (80.0, 80.0);
/// The two window sizes the resize bench alternates between, in logical points.
const SIZES: [(f32, f32); 2] = [(1100.0, 712.0), (900.0, 600.0)];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Drag,
    Resize,
    Settled,
    Long,
    Zoom,
    Compare,
    Crop,
    Dedust,
    Flows,
    Presets,
    Prepare,
    Console,
    Sheet,
    Loupe,
}

impl Mode {
    fn notches(self) -> usize {
        if self == Mode::Long { LONG } else { NOTCHES }
    }

    /// The drag's travel at `notch`, in points.
    fn travel(self, notch: usize) -> f32 {
        notch as f32 * if self == Mode::Long { LONG_STEP } else { STEP }
    }
}

pub fn requested() -> Option<Mode> {
    match std::env::var("ONI_BENCH").ok()?.as_str() {
        "1" => Some(Mode::Drag),
        "resize" => Some(Mode::Resize),
        "settled" => Some(Mode::Settled),
        "long" => Some(Mode::Long),
        "zoom" => Some(Mode::Zoom),
        "compare" => Some(Mode::Compare),
        "crop" => Some(Mode::Crop),
        "dedust" => Some(Mode::Dedust),
        "flows" => Some(Mode::Flows),
        "presets" => Some(Mode::Presets),
        "prepare" => Some(Mode::Prepare),
        "console" => Some(Mode::Console),
        "sheet" => Some(Mode::Sheet),
        "loupe" => Some(Mode::Loupe),
        _ => None,
    }
}

#[derive(Clone, Copy)]
enum Phase {
    Loading,
    Dragging { notch: usize, from: usize },
    Landing { revision: u64, from: usize },
}

/// Drives the drag on the window's timers; `done` receives the report once the last notch is shown
/// and the drag is undone, so the frame's sidecar reads as it did.
pub fn start(ctl: &Rc<Ctl>, mode: Mode, done: impl FnOnce(String) + 'static) {
    if mode == Mode::Zoom {
        return zoom(ctl, done);
    }
    if mode == Mode::Crop {
        return crop(ctl, done);
    }
    if mode == Mode::Dedust {
        return dedust(ctl, done);
    }
    if mode == Mode::Flows {
        return flows(ctl, done);
    }
    if mode == Mode::Presets {
        return deck(ctl, done);
    }
    if mode == Mode::Console {
        return console(ctl, done);
    }
    if mode == Mode::Loupe {
        return loupe(ctl, done);
    }
    if mode == Mode::Prepare {
        return prepare(ctl, done);
    }
    if mode == Mode::Sheet {
        return sheet(ctl, done);
    }
    let timer = Rc::new(Timer::default());
    let phase = Cell::new(Phase::Loading);
    let done = RefCell::new(Some(done));
    let started = Instant::now();
    if let Some(w) = ctl.window() {
        w.window().set_position(slint::LogicalPosition::new(ORIGIN.0, ORIGIN.1));
    }
    let me = Rc::downgrade(ctl);
    let held = timer.clone();
    timer.start(TimerMode::Repeated, PACE, move || {
        let timer = &held;
        let Some(ctl) = me.upgrade() else { return timer.stop() };
        let Some(w) = ctl.window() else { return };
        let g = w.global::<StoreGlobal>();
        let field = Field::Exposure.index();
        let finish = |text: String| {
            timer.stop();
            if let Some(done) = done.borrow_mut().take() {
                done(text);
            }
        };
        match phase.get() {
            Phase::Loading => {
                let settled = !matches!(mode, Mode::Settled | Mode::Long) || ctl.state.borrow().loaded() >= ctl.state.borrow().photos();
                if ctl.state.borrow().renders() >= 1 && settled {
                    if mode == Mode::Compare && ctl.state.borrow().split().is_none() {
                        ctl.act("compare");
                        return;
                    }
                    if mode == Mode::Compare && ctl.state.borrow().befores() == 0 {
                        return;
                    }
                    let from = ctl.state.borrow().notches().count();
                    g.invoke_press(field, false);
                    phase.set(Phase::Dragging { notch: 0, from });
                } else if started.elapsed() > LOADING {
                    finish(format!("bench: no frame rendered in {} s", LOADING.as_secs()));
                }
            }
            Phase::Dragging { notch, from } => {
                let notch = notch + 1;
                g.invoke_drag(field, mode.travel(notch), SPAN, false);
                if mode == Mode::Compare {
                    let width = w.global::<crate::Canvas>().get_width();
                    w.global::<crate::Canvas>().invoke_split_dragged(width * (0.2 + 0.6 * notch as f32 / NOTCHES as f32));
                }
                if mode == Mode::Resize {
                    let (width, height) = SIZES[notch % SIZES.len()];
                    w.window().set_size(slint::LogicalSize::new(width, height));
                }
                if notch < mode.notches() {
                    phase.set(Phase::Dragging { notch, from });
                } else {
                    g.invoke_release(field);
                    phase.set(Phase::Landing { revision: ctl.state.borrow().store().revision(), from });
                }
            }
            Phase::Landing { revision, from } => {
                if ctl.state.borrow().rendered() < revision {
                    return;
                }
                let title = match mode {
                    Mode::Drag => format!("bench, {NOTCHES} notches dragged on Exposure"),
                    Mode::Resize => format!("bench, {NOTCHES} notches dragged on Exposure, the window resized on each"),
                    Mode::Settled => format!("bench, {NOTCHES} notches dragged on Exposure, every thumbnail read"),
                    Mode::Long | Mode::Zoom | Mode::Crop | Mode::Dedust | Mode::Flows | Mode::Presets | Mode::Prepare | Mode::Console | Mode::Sheet | Mode::Loupe => format!("bench, {LONG} notches dragged on Exposure for ten seconds"),
                    Mode::Compare => format!("bench, {NOTCHES} notches dragged on Exposure beside Compare's before"),
                };
                let notches = ctl.state.borrow().notches().summary(from, &title);
                let mut report = format!("{notches}\n{}", ctl.state.borrow().notches().cadence(from));
                if mode == Mode::Compare {
                    let s = ctl.state.borrow();
                    report.push_str(&format!("\ncompare: {} before rendered over the drag, split at {:.2}", s.befores(), s.split().unwrap_or(-1.0)));
                }
                ctl.act("undo");
                ctl.flush();
                finish(report);
            }
        }
    });
}

/// Sixty pan steps of 6 points, a trackpad's glide, once at 100 %.
const PANS: usize = 60;
const PAN_STEP: f32 = 6.0;
/// What full resolution may take to arrive, decode included.
const FULL: Duration = Duration::from_secs(20);

#[derive(Clone, Copy)]
enum Zoomed {
    Loading,
    In { asked: Instant },
    Panning { step: usize, from: usize },
    Out { from: usize, full_after: Duration },
}

/// The canvas's own callbacks driven as a hand would: the double click, the drag, the double click back.
fn zoom(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
    let timer = Rc::new(Timer::default());
    let phase = Cell::new(Zoomed::Loading);
    let done = RefCell::new(Some(done));
    let full_after = Cell::new(Duration::ZERO);
    let me = Rc::downgrade(ctl);
    let held = timer.clone();
    timer.start(TimerMode::Repeated, PACE, move || {
        let timer = &held;
        let Some(ctl) = me.upgrade() else { return timer.stop() };
        let Some(w) = ctl.window() else { return };
        let c = w.global::<crate::Canvas>();
        let finish = |text: String| {
            timer.stop();
            if let Some(done) = done.borrow_mut().take() {
                done(text);
            }
        };
        let s = ctl.state.borrow();
        match phase.get() {
            Zoomed::Loading => {
                if s.renders() >= 1 && s.loaded() >= s.photos() && s.placed().is_some() {
                    drop(s);
                    c.invoke_double_clicked();
                    phase.set(Zoomed::In { asked: Instant::now() });
                }
            }
            Zoomed::In { asked } => {
                let full = s.placed().is_some_and(|p| p.at_full_resolution);
                if full {
                    full_after.set(asked.elapsed());
                    let from = s.views().count();
                    drop(s);
                    c.invoke_grabbed(200.0, 200.0);
                    phase.set(Zoomed::Panning { step: 0, from });
                } else if asked.elapsed() > FULL {
                    drop(s);
                    finish(format!("bench, zoom: no full-resolution render in {} s", FULL.as_secs()));
                }
            }
            Zoomed::Panning { step, from } => {
                drop(s);
                let step = step + 1;
                let x = 200.0 - step as f32 * PAN_STEP;
                c.invoke_dragged(x, 200.0 - step as f32 * PAN_STEP / 2.0);
                if step < PANS {
                    phase.set(Zoomed::Panning { step, from });
                } else {
                    c.invoke_double_clicked();
                    phase.set(Zoomed::Out { from, full_after: full_after.get() });
                }
            }
            Zoomed::Out { from, full_after } => {
                let fitted = s.viewport().is_fitted() && s.placed().is_some_and(|p| p.whole);
                if !fitted {
                    return;
                }
                let report = format!(
                    "bench, zoom: full resolution {:.0} ms after the double click; {}\n{}",
                    full_after.as_secs_f64() * 1000.0,
                    s.views().summary(from, &format!("{PANS} pan steps at 100 %")),
                    s.views().cadence(from)
                );
                drop(s);
                finish(report);
            }
        }
    });
}

/// The Crop tab's keys and the canvas's crop frame driven through their own callbacks, each step's
/// geometry read back, then every step undone.
fn crop(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
    let timer = Rc::new(Timer::default());
    let step = Cell::new(0usize);
    let done = RefCell::new(Some(done));
    let lines = RefCell::new(Vec::<String>::new());
    let start = RefCell::new(None::<oni_settings::frame::Geometry>);
    let me = Rc::downgrade(ctl);
    let held = timer.clone();
    timer.start(TimerMode::Repeated, Duration::from_millis(250), move || {
        let timer = &held;
        let Some(ctl) = me.upgrade() else { return timer.stop() };
        let Some(w) = ctl.window() else { return };
        let (f, c, panel) = (w.global::<crate::Framing>(), w.global::<crate::Canvas>(), w.global::<crate::PanelState>());
        let geometry = || ctl.state.borrow().store().settings().geometry.clone();
        let note = |what: &str| {
            let g = geometry();
            lines.borrow_mut().push(format!("{what}: crop {:.3} {:.3} {:.3} {:.3}, turns {}, angle {:.2}, mirrored {}, ratio {:?}, kept {}", g.crop.x, g.crop.y, g.crop.width, g.crop.height, g.quarter_turns, g.angle, g.mirrored, g.ratio, f.get_kept()));
        };
        let n = step.get();
        step.set(n + 1);
        match n {
            0 => {
                if ctl.state.borrow().renders() < 1 || ctl.state.borrow().placed().is_none() {
                    step.set(0);
                    return;
                }
                *start.borrow_mut() = Some(geometry());
                panel.set_tab(1);
                panel.invoke_tab_picked(1);
            }
            1 => {
                note(&format!("framing {}", if c.get_cropping() { "on" } else { "off" }));
                f.invoke_ratio_picked(3);
            }
            2 => {
                note("1:1");
                f.invoke_act("rotate".into());
            }
            3 => {
                note("rotated");
                f.invoke_act("mirror".into());
            }
            4 => {
                note("mirrored");
                // `CropOverlay`'s Esc: the held corner back where the press found it, the rest of the drag spent.
                let pressed = geometry().crop;
                c.invoke_frame_grabbed(false, 4);
                c.invoke_frame_dragged(false, 4, -40.0, -20.0);
                let moved = geometry().crop != pressed;
                let escape: slint::SharedString = slint::platform::Key::Escape.into();
                w.window().dispatch_event(slint::platform::WindowEvent::KeyPressed { text: escape.clone() });
                w.window().dispatch_event(slint::platform::WindowEvent::KeyReleased { text: escape });
                c.invoke_frame_dragged(false, 4, -60.0, -30.0);
                c.invoke_frame_released(false);
                lines.borrow_mut().push(format!("Esc in a drag that moved the corner ({moved}) put the frame back: {}", geometry().crop == pressed));
                c.invoke_frame_grabbed(false, 4);
                for k in 1..=10 {
                    c.invoke_frame_dragged(false, 4, -4.0 * k as f32, -2.0 * k as f32);
                }
                c.invoke_frame_released(false);
            }
            5 => {
                note("bottom-right corner dragged in");
                let g = w.global::<StoreGlobal>();
                let field = Field::Angle.index();
                g.invoke_press(field, false);
                for k in 1..=10 {
                    g.invoke_drag(field, k as f32 * 2.0, SPAN, false);
                }
                g.invoke_release(field);
            }
            6 => note("straightened"),
            7..=12 => ctl.act("undo"),
            13 => {
                note("undone");
                let back = start.borrow().clone() == Some(geometry());
                timer.stop();
                panel.set_tab(2);
                panel.invoke_tab_picked(2);
                let report = format!("bench, crop:\n{}\nundone to the frame it started from: {back}", lines.borrow().join("\n"));
                if let Some(done) = done.borrow_mut().take() {
                    done(report);
                }
            }
            _ => {}
        }
    });
}

/// De-dust through its own key, the brush keys and the canvas's painting callbacks; the patch
/// waited for, then the correction removed and the tool left.
fn dedust(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
    let timer = Rc::new(Timer::default());
    let step = Cell::new(0usize);
    let done = RefCell::new(Some(done));
    let lines = RefCell::new(Vec::<String>::new());
    let painted = Cell::new(None::<Instant>);
    let me = Rc::downgrade(ctl);
    let held = timer.clone();
    timer.start(TimerMode::Repeated, Duration::from_millis(200), move || {
        let timer = &held;
        let Some(ctl) = me.upgrade() else { return timer.stop() };
        let Some(w) = ctl.window() else { return };
        let (d, c) = (w.global::<crate::Dedust>(), w.global::<crate::Canvas>());
        let corrections = || ctl.state.borrow().store().settings().corrections.clone();
        let n = step.get();
        step.set(n + 1);
        match n {
            0 => {
                if ctl.state.borrow().renders() < 1 || ctl.state.borrow().placed().is_none() {
                    step.set(0);
                    return;
                }
                ctl.act("correction");
                ctl.act("bigger-brush");
                ctl.act("bigger-brush");
            }
            1 => {
                lines.borrow_mut().push(format!("painting {}, brush {}", c.get_painting(), d.get_readout()));
                // A short stroke across the middle of the frame shown.
                let (x, y) = (c.get_frame_x() + c.get_frame_width() * 0.4, c.get_frame_y() + c.get_frame_height() * 0.45);
                c.invoke_paint_start(x, y);
                for k in 1..=8 {
                    c.invoke_paint_move(x + k as f32 * 3.0, y + k as f32);
                }
                c.invoke_paint_end();
                painted.set(Some(Instant::now()));
                let list = corrections();
                lines.borrow_mut().push(format!("{} correction, radius {:.0}, {} points, marks {}", list.len(), list.first().map_or(0.0, |c| c.radius), list.first().map_or(0, |c| c.stroke.len()), c.get_marks().row_count()));
            }
            2 => {
                let list = corrections();
                let Some(first) = list.first() else { return finish(timer, &done, &lines, "no correction to wait on") };
                if !crate::patches::file(&crate::patches::caches(), first.id).exists() {
                    if painted.get().is_some_and(|t| t.elapsed() > Duration::from_secs(30)) {
                        return finish(timer, &done, &lines, "the patch never came");
                    }
                    step.set(2);
                    return;
                }
                let row = d.get_rows().row_data(0);
                lines.borrow_mut().push(format!(
                    "patch generated {:.0} ms after the stroke; row {:?}, pending {:?}",
                    painted.get().map_or(0.0, |t| t.elapsed().as_secs_f64() * 1000.0),
                    row.as_ref().map(|r| r.label.to_string()),
                    row.as_ref().map(|r| r.pending)
                ));
            }
            3 => {
                let id = corrections().first().map(|c| c.id);
                d.invoke_remove(0);
                let gone = id.is_some_and(|id| !crate::patches::file(&crate::patches::caches(), id).exists());
                lines.borrow_mut().push(format!("removed: {} corrections left, patch gone {gone}", corrections().len()));
                ctl.act("correction");
                lines.borrow_mut().push(format!("left De-dust: painting {}", c.get_painting()));
                ctl.flush();
                finish(timer, &done, &lines, "done");
            }
            _ => {}
        }
    });
}

fn finish(timer: &Timer, done: &RefCell<Option<impl FnOnce(String)>>, lines: &RefCell<Vec<String>>, last: &str) {
    timer.stop();
    if let Some(done) = done.borrow_mut().take() {
        done(format!("bench, dedust:\n{}\n{last}", lines.borrow().join("\n")));
    }
}

/// The frames the flow opens from the gallery, by their place in it.
const FLOW_TILES: [usize; 3] = [1, 4, 7];

#[derive(Clone, Copy)]
enum Flow {
    Loading,
    Gallery { frame: usize },
    Picked { frame: usize },
    Opening { frame: usize, at: Instant },
    Editing { frame: usize, notch: usize },
    Shown { frame: usize, revision: u64 },
    Back { frame: usize },
    Below { name: usize },
}

/// A press and a release where the hand would put them, through the window's own event path.
fn click_at(w: &crate::MainWindow, at: slint::LogicalPosition) {
    use slint::platform::{PointerEventButton, WindowEvent};
    let window = w.window();
    window.dispatch_event(WindowEvent::PointerMoved { position: at });
    window.dispatch_event(WindowEvent::PointerPressed { position: at, button: PointerEventButton::Left });
    window.dispatch_event(WindowEvent::PointerReleased { position: at, button: PointerEventButton::Left });
}

/// The tile at place `i` in the gallery, when it is laid on screen.
fn tile(w: &crate::MainWindow, i: usize) -> Option<crate::Tile> {
    let lib = w.global::<crate::Library>();
    let tiles = lib.get_tiles();
    (0..tiles.row_count()).filter_map(|k| tiles.row_data(k)).find(|t| t.index as usize == i)
}

/// The tiles and the sidebar's photo rows lit, by name.
fn lit(w: &crate::MainWindow) -> (Vec<String>, Vec<String>) {
    let lib = w.global::<crate::Library>();
    let tiles = lib.get_tiles();
    let rows = lib.get_rows();
    (
        (0..tiles.row_count()).filter_map(|k| tiles.row_data(k)).filter(|t| t.selected).map(|t| t.caption.to_string()).collect(),
        (0..rows.row_count()).filter_map(|k| rows.row_data(k)).filter(|r| r.kind == 2 && r.selected).map(|r| r.title.to_string()).collect(),
    )
}

fn flows(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
    let timer = Rc::new(Timer::default());
    let phase = Cell::new(Flow::Loading);
    let done = RefCell::new(Some(done));
    let failures = RefCell::new(Vec::<String>::new());
    let started = Instant::now();
    let me = Rc::downgrade(ctl);
    let held = timer.clone();
    timer.start(TimerMode::Repeated, Duration::from_millis(50), move || {
        let timer = &held;
        let Some(ctl) = me.upgrade() else { return timer.stop() };
        let Some(w) = ctl.window() else { return };
        let app = w.global::<crate::App>();
        let lib = w.global::<crate::Library>();
        let finish = |text: String| {
            timer.stop();
            if let Some(done) = done.borrow_mut().take() {
                done(text);
            }
        };
        let fail = |what: String| failures.borrow_mut().push(what);
        let centre = |t: &crate::Tile| slint::LogicalPosition::new(lib.get_grid_x() + t.x + t.width / 2.0, lib.get_grid_y() + t.y - lib.get_scroll() + t.height / 2.0);
        // One name lit in the grid and the same one, alone, in the sidebar.
        let check = |name: &str, when: &str| {
            let (tiles, rows) = lit(&w);
            if tiles != [name] || rows != [name] {
                fail(format!("{name} {when}: the gallery lights {tiles:?}, the sidebar {rows:?}"));
            }
        };
        match phase.get() {
            Flow::Loading => {
                if ctl.state.borrow().renders() >= 1 && ctl.state.borrow().placed().is_some() {
                    ctl.act("toggle-mode");
                    phase.set(Flow::Gallery { frame: 0 });
                } else if started.elapsed() > LOADING {
                    finish("flows: no frame rendered".into());
                }
            }
            Flow::Gallery { frame } => {
                let Some(t) = tile(&w, FLOW_TILES[frame]).filter(|_| app.get_gallery() && lib.get_grid_x() > 0.0) else { return };
                // The double-click interval apart from the last click, so this one starts afresh.
                click_at(&w, centre(&t));
                phase.set(Flow::Picked { frame });
            }
            Flow::Picked { frame } => {
                let Some(t) = tile(&w, FLOW_TILES[frame]) else { return };
                let name = t.caption.to_string();
                if !app.get_gallery() {
                    fail(format!("{name}: one click left the gallery"));
                }
                check(&name, "clicked once");
                // The second click of a double click, inside the system's interval.
                click_at(&w, centre(&t));
                phase.set(Flow::Opening { frame, at: Instant::now() });
            }
            Flow::Opening { frame, at } => {
                let name = tile(&w, FLOW_TILES[frame]).map(|t| t.caption.to_string()).unwrap_or_default();
                let open = ctl.state.borrow().open_path().is_some_and(|p| p.contains(&name));
                if !app.get_gallery() && open && ctl.state.borrow().placed().is_some() {
                    check(&name, "opened");
                    w.global::<StoreGlobal>().invoke_press(Field::Exposure.index(), false);
                    phase.set(Flow::Editing { frame, notch: 0 });
                } else if at.elapsed() > Duration::from_secs(60) {
                    fail(format!("{name}: a double click did not open it (gallery {}, open {:?})", app.get_gallery(), ctl.state.borrow().open_path()));
                    finish(format!("flows: {}", failures.borrow().join("; ")));
                }
            }
            Flow::Editing { frame, notch } => {
                let g = w.global::<StoreGlobal>();
                let notch = notch + 1;
                g.invoke_drag(Field::Exposure.index(), notch as f32 * STEP, SPAN, false);
                // The window resized mid-edit, as entering the editor resizes the canvas.
                let (width, height) = SIZES[notch % SIZES.len()];
                w.window().set_size(slint::LogicalSize::new(width, height));
                if notch < 12 {
                    phase.set(Flow::Editing { frame, notch });
                } else {
                    g.invoke_release(Field::Exposure.index());
                    phase.set(Flow::Shown { frame, revision: ctl.state.borrow().store().revision() });
                }
            }
            Flow::Shown { frame, revision } => {
                if ctl.state.borrow().rendered() < revision {
                    return;
                }
                // The edit undone, so the test copy's sidecar reads as it did.
                ctl.act("undo");
                ctl.act("toggle-mode");
                phase.set(Flow::Back { frame });
            }
            Flow::Back { frame } => {
                let Some(t) = tile(&w, FLOW_TILES[frame]).filter(|_| app.get_gallery()) else { return };
                check(&t.caption, "back in the gallery");
                if frame + 1 < FLOW_TILES.len() {
                    phase.set(Flow::Gallery { frame: frame + 1 });
                } else {
                    // Under the last row, where the document ends: the picks stay as they were.
                    let below = slint::LogicalPosition::new(lib.get_grid_x() + 4.0, lib.get_grid_y() + lib.get_content_height() - lib.get_scroll() + 4.0);
                    click_at(&w, below);
                    phase.set(Flow::Below { name: FLOW_TILES[frame] });
                }
            }
            Flow::Below { name } => {
                if let Some(t) = tile(&w, name) {
                    check(&t.caption, "after a click under the last row");
                }
                let failed = failures.borrow();
                finish(if failed.is_empty() {
                    format!("flows: {} frames picked, double-clicked open and edited through resizes, a click under the rows changed nothing, no failure", FLOW_TILES.len())
                } else {
                    format!("flows: {}", failed.join("; "))
                });
            }
        }
    });
}

#[derive(Clone, Copy)]
enum Deck {
    Loading,
    Saved { at: Instant },
    Raised { at: Instant },
    Hovered { at: Instant },
    Applied,
}

fn pointer_to(w: &crate::MainWindow, at: slint::LogicalPosition) {
    w.window().dispatch_event(slint::platform::WindowEvent::PointerMoved { position: at });
}

fn deck(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
    let timer = Rc::new(Timer::default());
    let phase = Cell::new(Deck::Loading);
    let done = RefCell::new(Some(done));
    let started = Instant::now();
    let saved = Cell::new(0.0f32);
    let me = Rc::downgrade(ctl);
    let held = timer.clone();
    timer.start(TimerMode::Repeated, Duration::from_millis(50), move || {
        let timer = &held;
        let Some(ctl) = me.upgrade() else { return timer.stop() };
        let Some(w) = ctl.window() else { return };
        let presets = w.global::<crate::Presets>();
        let finish = |text: String| {
            timer.stop();
            if let Some(done) = done.borrow_mut().take() {
                done(text);
            }
        };
        let exposure = || ctl.state.borrow().store().get(Field::Exposure);
        let shown = || ctl.state.borrow().shown_settings().map(|s| Field::Exposure.knob().get(s));
        // The card's middle, wherever the deck stands now.
        let card = || slint::LogicalPosition::new(presets.get_first_x() + 36.0, presets.get_first_y() + 52.0);
        match phase.get() {
            Deck::Loading => {
                if ctl.state.borrow().renders() < 1 {
                    if started.elapsed() > LOADING {
                        finish("presets: no frame rendered".into());
                    }
                    return;
                }
                // The look saved two stops over the frame's own, then the frame back at rest.
                let rest = exposure();
                let g = w.global::<StoreGlobal>();
                g.invoke_press(Field::Exposure.index(), false);
                g.invoke_drag(Field::Exposure.index(), 30.0, SPAN, false);
                g.invoke_release(Field::Exposure.index());
                saved.set(exposure());
                ctl.act("save-preset");
                presets.invoke_form_named("Bench".into());
                presets.invoke_save_confirmed();
                ctl.set(Field::Exposure, rest);
                phase.set(Deck::Saved { at: Instant::now() });
            }
            Deck::Saved { at } => {
                if at.elapsed() < Duration::from_millis(300) {
                    return;
                }
                // Onto the strip the deck leaves at rest: the deck rises under the pointer.
                let bottom = presets.get_first_y() + 8.0;
                pointer_to(&w, slint::LogicalPosition::new(presets.get_first_x() + 36.0, bottom));
                phase.set(Deck::Raised { at: Instant::now() });
            }
            Deck::Raised { at } => {
                if at.elapsed() < Duration::from_millis(400) {
                    return;
                }
                pointer_to(&w, card());
                phase.set(Deck::Hovered { at: Instant::now() });
            }
            Deck::Hovered { at } => {
                let previewing = presets.get_hovered() == 0 && shown().is_some_and(|e| (e - saved.get()).abs() < 1e-4);
                if previewing && (exposure() - saved.get()).abs() > 1e-3 {
                    let at = card();
                    let window = w.window();
                    window.dispatch_event(slint::platform::WindowEvent::PointerPressed { position: at, button: slint::platform::PointerEventButton::Left });
                    window.dispatch_event(slint::platform::WindowEvent::PointerReleased { position: at, button: slint::platform::PointerEventButton::Left });
                    phase.set(Deck::Applied);
                } else if at.elapsed() > Duration::from_secs(20) {
                    finish(format!(
                        "presets: no preview under the pointer (hovered {}, shown {:?}, saved {}, store {})",
                        presets.get_hovered(),
                        shown(),
                        saved.get(),
                        exposure()
                    ));
                }
            }
            Deck::Applied => {
                let applied = (exposure() - saved.get()).abs() < 1e-4;
                // Taken back, so the test copy's sidecar reads as it did.
                ctl.act("undo");
                finish(if applied {
                    format!("presets: the deck rose, the card previewed {:.2} EV under the pointer and its click applied it, no failure", saved.get())
                } else {
                    format!("presets: the click applied nothing (store {}, saved {})", exposure(), saved.get())
                });
            }
        }
    });
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Prep {
    Loading,
    Picked,
    Entering,
    Walking,
    Base,
    Flags,
    Window,
    Measuring,
    Landed,
}

/// The panel's step key, at the footer's trailing end.
fn action_key(p: &crate::Preparing) -> slint::LogicalPosition {
    slint::LogicalPosition::new(p.get_footer_x() + p.get_footer_width() - 12.0, p.get_footer_y() + p.get_footer_height() / 2.0)
}

fn prepare(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
    let timer = Rc::new(Timer::default());
    let phase = Cell::new(Prep::Loading);
    let done = RefCell::new(Some(done));
    let lines = RefCell::new(Vec::<String>::new());
    let started = Instant::now();
    let since = Cell::new(Instant::now());
    let me = Rc::downgrade(ctl);
    let held = timer.clone();
    timer.start(TimerMode::Repeated, Duration::from_millis(50), move || {
        let timer = &held;
        let Some(ctl) = me.upgrade() else { return timer.stop() };
        let Some(w) = ctl.window() else { return };
        let p = w.global::<crate::Preparing>();
        let lib = w.global::<crate::Library>();
        let finish = |text: String| {
            timer.stop();
            if let Some(done) = done.borrow_mut().take() {
                done(format!("bench, prepare:\n{}\n{text}", lines.borrow().join("\n")));
            }
        };
        let note = |what: String| {
            lines.borrow_mut().push(format!("{what}: {:.0} ms", since.get().elapsed().as_secs_f64() * 1000.0));
            since.set(Instant::now());
        };
        let stuck = |what: &str| since.get().elapsed() > Duration::from_secs(120) && { finish(format!("prepare: stuck {what} at step {}", p.get_step())); true };
        match phase.get() {
            Prep::Loading => {
                if ctl.state.borrow().renders() >= 1 {
                    ctl.act("toggle-mode");
                    since.set(Instant::now());
                    phase.set(Prep::Picked);
                } else if started.elapsed() > LOADING {
                    finish("prepare: no frame rendered".into());
                }
            }
            Prep::Picked => {
                let Some(t) = tile(&w, 0).filter(|_| lib.get_grid_x() > 0.0) else { return };
                click_at(&w, slint::LogicalPosition::new(lib.get_grid_x() + t.x + t.width / 2.0, lib.get_grid_y() + t.y - lib.get_scroll() + t.height / 2.0));
                w.global::<crate::Bar>().invoke_act("prepare-roll".into());
                phase.set(Prep::Entering);
            }
            Prep::Entering => {
                if p.get_shown() && ctl.state.borrow().prepare.read_whole() {
                    note(format!("{} frames entered and read", p.get_cells().row_count()));
                    p.invoke_step_picked(0);
                    phase.set(Prep::Walking);
                } else {
                    stuck("entering");
                }
            }
            Prep::Walking => {
                if p.get_step() == 2 {
                    phase.set(Prep::Base);
                } else {
                    click_at(&w, action_key(&p));
                }
            }
            Prep::Base => {
                if !p.get_scan_ready() {
                    stuck("on the base's scan");
                    return;
                }
                note("the base's scan shown".into());
                // The scan's top-left rebate, where a holder leaves bare film.
                let at = slint::LogicalPosition::new(p.get_base_origin_x() + p.get_scan_x() + p.get_scan_width() * 0.02, p.get_base_origin_y() + p.get_scan_y() + p.get_scan_height() * 0.5);
                pointer_to(&w, at);
                click_at(&w, at);
                phase.set(Prep::Flags);
            }
            Prep::Flags => {
                if p.get_step() != 3 {
                    stuck("picking the base");
                    return;
                }
                note(format!("base picked, {}", p.get_base_note()));
                let width = (p.get_sheet_width() - 2.0 * 24.0 - 5.0 * 16.0) / 6.0;
                click_at(&w, slint::LogicalPosition::new(p.get_sheet_origin_x() + 24.0 + width / 2.0, p.get_sheet_origin_y() + 24.0 + width / 3.0));
                lines.borrow_mut().push(format!("flagged the first frame: {}", p.get_flags_note()));
                click_at(&w, action_key(&p));
                phase.set(Prep::Window);
            }
            Prep::Window => {
                if p.get_step() != 4 {
                    stuck("reaching the window");
                    return;
                }
                lines.borrow_mut().push(p.get_action_title().to_string());
                since.set(Instant::now());
                click_at(&w, action_key(&p));
                phase.set(Prep::Measuring);
            }
            Prep::Measuring => {
                if !p.get_shown() {
                    let app = w.global::<crate::App>();
                    note(format!("measured and landed, the editor {}", if app.get_gallery() { "not open" } else { "open" }));
                    phase.set(Prep::Landed);
                } else if !stuck("measuring") && w.global::<crate::Advice>().get_shown() {
                    finish(format!("prepare: {}", w.global::<crate::Advice>().get_message()));
                }
            }
            Prep::Landed => {
                ctl.act("toggle-mode");
                let label = w.global::<crate::Menus>().get_titles().undo.to_string();
                // Taken back, so the test copy reads its own measure again.
                ctl.act("undo");
                finish(format!("prepare: {label} landed and undone in {:.1} s, no failure", started.elapsed().as_secs_f64()));
            }
        }
    });
}

fn console(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
    let timer = Rc::new(Timer::default());
    let done = RefCell::new(Some(done));
    let started = Instant::now();
    let me = Rc::downgrade(ctl);
    let held = timer.clone();
    ctl.act("console");
    timer.start(TimerMode::Repeated, Duration::from_millis(100), move || {
        let Some(ctl) = me.upgrade() else { return held.stop() };
        let shown = ctl.console_log();
        let report = match shown {
            Some(log) if log.contains("GPU:") => format!("console: the window shows the session's log, {} lines, no failure", log.lines().count()),
            _ if started.elapsed() > Duration::from_secs(10) => format!("console: the log never showed ({:?} characters)", shown.map(|l| l.len())),
            _ => return,
        };
        held.stop();
        if let Some(done) = done.borrow_mut().take() {
            done(report);
        }
    });
}

/// The gallery's first six frames picked by the hand's clicks, then New Contact Sheet from the bar.
fn sheet(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
    let timer = Rc::new(Timer::default());
    let done = RefCell::new(Some(done));
    let asked: Cell<Option<(Instant, usize)>> = Cell::new(None);
    let started = Instant::now();
    let me = Rc::downgrade(ctl);
    let held = timer.clone();
    timer.start(TimerMode::Repeated, Duration::from_millis(50), move || {
        let Some(ctl) = me.upgrade() else { return held.stop() };
        let Some(w) = ctl.window() else { return };
        let finish = |text: String| {
            held.stop();
            if let Some(done) = done.borrow_mut().take() {
                done(format!("bench, sheet: {text}"));
            }
        };
        let advice = w.global::<crate::Advice>();
        match asked.get() {
            None => {
                if ctl.state.borrow().renders() < 1 {
                    return;
                }
                ctl.act("toggle-mode");
                let lib = w.global::<crate::Library>();
                let mut tiles = crate::app::tiles(&w);
                tiles.sort_by_key(|t| t.index);
                for (i, t) in tiles.iter().filter(|t| !t.caption.contains("contact sheet")).take(6).enumerate() {
                    lib.invoke_tile_clicked(t.index, i > 0, false);
                }
                let title = w.global::<crate::Menus>().get_titles().sheet.to_string();
                w.global::<crate::Bar>().invoke_act("contact-sheet".into());
                asked.set(Some((Instant::now(), ctl.state.borrow().renders())));
                eprintln!("bench, sheet: {title} asked");
            }
            Some((at, renders)) => {
                if advice.get_shown() {
                    return finish(format!("{}: {}", advice.get_title(), advice.get_message()));
                }
                let open = ctl.state.borrow().open_path().map(str::to_string);
                // Done once the sheet is open and the canvas has drawn since the ask.
                if let Some(path) = open.filter(|p| p.contains("contact sheet")) && ctl.state.borrow().renders() > renders {
                    return finish(format!("{path} made, opened and drawn in {:.1} s, no failure", at.elapsed().as_secs_f64()));
                }
                if started.elapsed() > LOADING {
                    finish("no sheet opened".into());
                }
            }
        }
    });
}

/// Sharpening notches while the Details loupe shows the frame at its own scale.
const LOUPE_NOTCHES: usize = 10;

/// The Details tab on the open frame: its full resolution built and the loupe's first render, then
/// each Sharpening notch until the loupe shows it, the badge's 200 %, all undone.
fn loupe(ctl: &Rc<Ctl>, done: impl FnOnce(String) + 'static) {
    let timer = Rc::new(Timer::default());
    let step = Cell::new(0usize);
    let done = RefCell::new(Some(done));
    let asked = Cell::new(None::<(Instant, u64)>);
    let times = RefCell::new(Vec::<f64>::new());
    let lines = RefCell::new(Vec::<String>::new());
    let start = Cell::new(0.0f32);
    let me = Rc::downgrade(ctl);
    let held = timer.clone();
    timer.start(TimerMode::Repeated, Duration::from_millis(5), move || {
        let timer = &held;
        let Some(ctl) = me.upgrade() else { return timer.stop() };
        let Some(w) = ctl.window() else { return };
        let (panel, band, g) = (w.global::<crate::PanelState>(), w.global::<crate::DetailLoupe>(), w.global::<StoreGlobal>());
        let loupes = ctl.state.borrow().loupes();
        let n = step.get();
        let landed = |since: Option<(Instant, u64)>| since.filter(|(_, before)| loupes > *before).map(|(at, _)| at.elapsed().as_secs_f64() * 1000.0);
        match n {
            0 => {
                if ctl.state.borrow().renders() < 1 {
                    return;
                }
                start.set(ctl.state.borrow().store().get(Field::SharpenAmount));
                panel.set_tab(3);
                panel.invoke_tab_picked(3);
                asked.set(Some((Instant::now(), loupes)));
                step.set(1);
            }
            1 => {
                let Some(ms) = landed(asked.get()) else { return };
                let size = band.get_source().size();
                lines.borrow_mut().push(format!("first loupe, full resolution built: {ms:.0} ms, {} × {} source px at {}", size.width, size.height, band.get_ratio()));
                step.set(2);
            }
            k if k >= 2 && k < 2 + 2 * LOUPE_NOTCHES => {
                if k % 2 == 0 {
                    g.invoke_step(Field::SharpenAmount.index(), 1);
                    asked.set(Some((Instant::now(), loupes)));
                    step.set(k + 1);
                } else if let Some(ms) = landed(asked.get()) {
                    times.borrow_mut().push(ms);
                    step.set(k + 1);
                }
            }
            k if k == 2 + 2 * LOUPE_NOTCHES => {
                band.invoke_toggled();
                asked.set(Some((Instant::now(), loupes)));
                step.set(k + 1);
            }
            k if k == 3 + 2 * LOUPE_NOTCHES => {
                let Some(ms) = landed(asked.get()) else { return };
                let size = band.get_source().size();
                lines.borrow_mut().push(format!("200 %: {ms:.0} ms, {} × {} source px at {}", size.width, size.height, band.get_ratio()));
                band.invoke_toggled();
                for _ in 0..LOUPE_NOTCHES {
                    if ctl.state.borrow().store().get(Field::SharpenAmount) == start.get() {
                        break;
                    }
                    ctl.act("undo");
                }
                let back = ctl.state.borrow().store().get(Field::SharpenAmount) == start.get();
                lines.borrow_mut().push(format!("undone to the amount it started from: {back}"));
                timer.stop();
                panel.set_tab(2);
                panel.invoke_tab_picked(2);
                let mut t = times.borrow().clone();
                t.sort_by(f64::total_cmp);
                let report = format!(
                    "bench, loupe:\n{}\na Sharpening notch reaches the loupe in {:.0} ms median, {:.0} ms worst, over {} notches",
                    lines.borrow().join("\n"),
                    t.get(t.len() / 2).copied().unwrap_or(f64::NAN),
                    t.last().copied().unwrap_or(f64::NAN),
                    t.len()
                );
                if let Some(done) = done.borrow_mut().take() {
                    done(report);
                }
            }
            _ => {}
        }
    });
}

