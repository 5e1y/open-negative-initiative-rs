//! `ONI_BENCH=1`: once the open frame is on the canvas, sixty notches dragged on {on} at a
//! trackpad's pace through the slider's own callbacks, then the notch centiles, without a hand.
//! `ONI_BENCH=resize` also resizes the window on every notch, so the surface is reconfigured mid-render;
//! `ONI_BENCH=settled` waits for every thumbnail first, so the drag has the GPU to itself;
//! `ONI_BENCH=long` then drags slowly for ten seconds and reports the image's cadence;
//! `ONI_BENCH=zoom` double-clicks to 100 %, waits for full resolution, pans, then walks the ladder to fit;
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
//! `ONI_BENCH=grid` opens up to 36 frames together and drags Exposure over them.
//! `ONI_BENCH=folded` pairs a phone over loopback and steps Exposure (`ONI_FOLDED=scanner`: the Scanner
//! keypad on Frontier) with the panel shown, then folded, counting each step rendered and drawn.

use crate::Store as StoreGlobal;
use crate::app::Ctl;
use crate::fields::Field;
use slint::{ComponentHandle, Model, Timer, TimerMode};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

mod zoom;
mod crop;
mod dedust;
mod flows;
mod presets;
mod prepare;
mod console;
mod sheet;
mod loupe;
mod folded;
mod grid;
use {zoom::*, crop::*, dedust::*, flows::*, presets::*, prepare::*, console::*, sheet::*, loupe::*, folded::*, grid::*};

pub const NOTCHES: usize = 60;

/// `ONI_BENCH_FIELD` names the dragged parameter by its Slint stem (`dye-separation`); Exposure else.
pub(crate) fn drag_field() -> Field {
    let named = std::env::var("ONI_BENCH_FIELD").ok();
    named.and_then(|n| Field::ALL.into_iter().find(|f| f.slint_name() == n)).unwrap_or(Field::Exposure)
}
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
    Folded,
    Grid,
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
        "folded" => Some(Mode::Folded),
        "grid" => Some(Mode::Grid),
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
    if mode == Mode::Folded {
        return folded(ctl, done);
    }
    if mode == Mode::Grid {
        return grid(ctl, done);
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
        let field = drag_field().index();
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
                let on = drag_field().slint_name();
                let title = match mode {
                    Mode::Drag => format!("bench, {NOTCHES} notches dragged on {on}"),
                    Mode::Resize => format!("bench, {NOTCHES} notches dragged on {on}, the window resized on each"),
                    Mode::Settled => format!("bench, {NOTCHES} notches dragged on {on}, every thumbnail read"),
                    Mode::Long | Mode::Zoom | Mode::Crop | Mode::Dedust | Mode::Flows | Mode::Presets | Mode::Prepare | Mode::Console | Mode::Sheet | Mode::Loupe | Mode::Folded | Mode::Grid => format!("bench, {LONG} notches dragged on {on} for ten seconds"),
                    Mode::Compare => format!("bench, {NOTCHES} notches dragged on {on} beside Compare's before"),
                };
                let notches = ctl.state.borrow().notches().summary(from, &title);
                let mut report = format!("{notches}\n{}\n{}", ctl.state.borrow().notches().cadence(from), ctl.state.borrow().notches().histograms());
                report = format!("{report}\n{}", ctl.state.borrow().notches().publishes());
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
