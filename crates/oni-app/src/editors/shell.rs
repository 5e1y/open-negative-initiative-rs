//! The editors in the window: each callback hands its gesture to `Gestures`, then the window draws
//! the hand's value at once and publishes the store only when it was written.

use super::geometry::{Dot, Drawing, Ink, Line, Size};
use super::sprung::Sprung;
use super::{Gestures, Plot, Reset, Then, curve, levels, print, saturation, spectrogram};
use crate::app::Ctl;
use crate::{Editors as Global, Ink as PlotInk, MainWindow, PlotDot, PlotKnob, PlotLine, PlotReadout};
use oni_settings::ConversionMode;
use oni_settings::curves::{Curve, SaturationCurve};
use oni_settings::levels::Levels;
use oni_settings::spectrogram::{Band, Spectrogram};
use oni_settings::tone::Print;
use slint::{Color, ComponentHandle, ModelRc, SharedString, Timer, TimerMode, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::time::Instant;

/// The editors' share of the window: their gestures, the trailing hand-off and what each plot last drew.
#[derive(Default)]
pub struct Shell {
    pub gestures: RefCell<Gestures>,
    trailing: Timer,
    /// Closes a lifted hand's step once the edit has rested.
    settling: Timer,
    drawn: RefCell<Drawn>,
    /// The open frame's path and whether it is an 8-bit scan, read once per frame.
    precision: RefCell<Option<(String, bool)>>,
    /// The levels handles and the band sliders' knobs, sprung to a value from elsewhere.
    knobs: RefCell<Knobs>,
    springing: Timer,
    /// The next drawing places every knob at once: a photo opened.
    jump: std::cell::Cell<bool>,
}

#[derive(Default)]
struct Knobs {
    levels: [Sprung; 5],
    bands: [Sprung; super::BAND_FIELDS],
    /// The track the handles were placed on: a new width lays them where they stand.
    levels_on: Option<Size>,
    /// A spring showed a frame short of its rest, so the next drawing lays the rest itself.
    levels_flying: bool,
    bands_flying: bool,
}

/// `FRAME_TICK`: a sprung knob is redrawn once a frame until it rests.
const FRAME: std::time::Duration = std::time::Duration::from_millis(16);

impl Shell {
    fn low_precision(&self, path: Option<&str>) -> bool {
        let Some(path) = path else { return false };
        let mut known = self.precision.borrow_mut();
        match known.as_ref() {
            Some((p, low)) if p == path => *low,
            _ => {
                let low = oni_decode::is_low_precision(std::path::Path::new(path));
                *known = Some((path.to_string(), low));
                low
            }
        }
    }
}

/// What each plot was last drawn from, so a notch elsewhere redraws nothing here.
#[derive(Default)]
struct Drawn {
    curves: Option<(Curve, usize, Size, Vec<bool>)>,
    levels: Option<(Levels, usize, ConversionMode, f32, Size, Size, Option<levels::Grip>, Vec<bool>, bool)>,
    wheel: Option<(Spectrogram, Size, Option<spectrogram::Grip>, Option<spectrogram::Grip>, Band, Option<(usize, f32)>)>,
    saturation: Option<(SaturationCurve, Size, Option<usize>)>,
    print: Option<(Print, bool, Size)>,
}

fn ink(i: Ink) -> PlotInk {
    match i {
        Ink::Guide => PlotInk::Guide,
        Ink::GuideFaint => PlotInk::GuideFaint,
        Ink::Marker => PlotInk::Marker,
        Ink::Separator => PlotInk::Separator,
        Ink::Active => PlotInk::Active,
        Ink::Idle => PlotInk::Idle,
        Ink::Track => PlotInk::Track,
        Ink::Primary => PlotInk::Primary,
        Ink::Canvas => PlotInk::Canvas,
        Ink::Black => PlotInk::Black,
        Ink::White => PlotInk::White,
        Ink::Red => PlotInk::Red,
        Ink::Green => PlotInk::Green,
        Ink::Blue => PlotInk::Blue,
        Ink::GreyQuarter => PlotInk::GreyQuarter,
        Ink::GreyHalf => PlotInk::GreyHalf,
        Ink::GreyThreeQuarters => PlotInk::GreyThreeQuarters,
        Ink::Band(b) => [
            PlotInk::BandRed,
            PlotInk::BandOrange,
            PlotInk::BandYellow,
            PlotInk::BandGreen,
            PlotInk::BandAqua,
            PlotInk::BandBlue,
            PlotInk::BandPurple,
            PlotInk::BandMagenta,
        ][b.min(7)],
    }
}

fn line(l: &Line) -> PlotLine {
    PlotLine { commands: l.commands.as_str().into(), ink: ink(l.ink), width: l.width as f32, opacity: l.opacity as f32 }
}

fn dot(d: &Dot) -> PlotDot {
    PlotDot {
        x: d.x as f32,
        y: d.y as f32,
        size: d.size as f32,
        fill: ink(d.fill),
        edge: ink(d.edge),
        edge_width: d.edge_width as f32,
        halo: d.halo as f32,
    }
}

fn model<T: Clone + 'static>(items: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(items))
}

fn lines(d: &Drawing) -> ModelRc<PlotLine> {
    model(d.lines.iter().map(line).collect())
}

fn dots(d: &Drawing) -> ModelRc<PlotDot> {
    model(d.dots.iter().map(dot).collect())
}

/// `Color(hue:saturation:brightness:)`, the band slider's spectrum ends.
pub fn hsb(hue: f64, saturation: f64, brightness: f64) -> (u8, u8, u8) {
    let h = (hue.rem_euclid(1.0)) * 6.0;
    let c = brightness * saturation;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = brightness - c;
    let byte = |v: f64| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (byte(r), byte(g), byte(b))
}

fn hue_end(band: Band, offset: f32) -> Color {
    let degrees = f64::from(band.centre() + offset).rem_euclid(360.0);
    let (r, g, b) = hsb(degrees / 360.0, 0.85, 1.0);
    Color::from_rgb_u8(r, g, b)
}

/// A left press anywhere in the window, in logical points, before the window takes it.
pub fn pressed_anywhere(ctl: &Rc<Ctl>, at: (f64, f64)) {
    if !ctl.editors.gestures.borrow_mut().pressed_anywhere(at) {
        return;
    }
    draw(ctl, false);
    if let Some(w) = ctl.window() {
        w.invoke_refocus();
    }
}

/// Every editor redrawn from the store and the hand, each only when what it reads moved; `jump`
/// places the knobs at once, as opening a photo does.
pub fn publish(ctl: &Rc<Ctl>, jump: bool) {
    if jump {
        ctl.editors.jump.set(true);
    }
    draw(ctl, false);
    // A channel tab moves the focus the input plots are drawn with.
    ctl.publish_inputs();
}

fn draw(ctl: &Rc<Ctl>, force: bool) {
    let Some(w) = ctl.window() else { return };
    let g = w.global::<Global>();
    let now = Instant::now();
    let (view, gestures, scanner, path, scrolled) = {
        let mut s = ctl.state.borrow_mut();
        let mut gestures = ctl.editors.gestures.borrow_mut();
        let scrolled = std::mem::take(&mut gestures.scrolled);
        let view = gestures.view(s.store.as_mut());
        let open = s.store.editor_ref();
        let (scanner, path) = (open.is_some_and(|e| e.scanner_face), open.and_then(|e| e.path()).map(str::to_string));
        (view, Snapshot::of(&gestures), scanner, path, scrolled)
    };
    let jump = ctl.editors.jump.take() || force;
    let mut knobs = ctl.editors.knobs.borrow_mut();
    let mut drawn = ctl.editors.drawn.borrow_mut();
    if force {
        *drawn = Drawn::default();
    }
    let sizes = gestures.sizes;
    let monochrome = view.mode.is_monochrome();
    g.set_monochrome(monochrome);
    g.set_scanner_face(scanner);
    g.set_low_precision(ctl.editors.low_precision(path.as_deref()));

    // The curves: the channel's spline, its points and the tabs of the moved channels.
    let channel = gestures.curve_channel;
    let curve_now = view.manual_curves.get(curve::CHANNELS[channel]).clone();
    let key = (curve_now, channel, sizes[Plot::Curves as usize], curve::marked(&view.manual_curves));
    if drawn.curves.as_ref() != Some(&key) {
        let d = curve::drawing(&key.0, curve::CHANNELS[channel], key.2);
        g.set_curve_lines(lines(&d));
        g.set_curve_dots(dots(&d));
        g.set_curve_marked(model(key.3.clone()));
        g.set_curve_channel(channel as i32);
        g.set_curves_modified(!view.manual_curves.is_neutral());
        drawn.curves = Some(key);
    }

    // The per-channel levels: handles, their markers on the histogram, readouts and tabs.
    let lc = levels::CHANNELS[gestures.levels_channel];
    let window = view.levels.get(lc);
    let modified = super::rested_levels(&view) != view.levels;
    let key = (
        window,
        gestures.levels_channel,
        view.mode,
        view.density_ceiling,
        sizes[Plot::Levels as usize],
        sizes[Plot::LevelsHistogram as usize],
        gestures.held_grip,
        levels::marked(&view.levels, view.mode, view.density_ceiling),
        modified,
    );
    let on = Some(key.4);
    let laid = std::mem::replace(&mut knobs.levels_on, on) != on;
    let changed = drawn.levels.as_ref() != Some(&key);
    if changed {
        for (k, grip) in levels::Grip::ALL.iter().enumerate() {
            let x = levels::knob_x(*grip, &window, lc, view.mode, key.4.w) as f32;
            let placed = jump || laid || scrolled || gestures.held_grip == Some(*grip);
            knobs.levels[k].aim(x, placed, now);
        }
    }
    let flying = knobs.levels.iter().any(|k| k.moving(now));
    let landing = std::mem::replace(&mut knobs.levels_flying, flying);
    if changed || flying || landing {
        let shown = levels::Grip::ALL
            .iter()
            .zip(knobs.levels)
            .map(|(grip, k)| PlotKnob {
                x: k.at(now),
                ink: ink(grip.tint()),
                held: gestures.held_grip == Some(*grip),
                label: grip.label().into(),
            })
            .collect();
        g.set_levels_knobs(model(shown));
    }
    if changed {
        g.set_levels_markers(lines(&levels::markers(&window, lc, view.mode, key.5)));
        let readouts = levels::readouts(&window, lc)
            .into_iter()
            .map(|(name, value)| PlotReadout { name: name.into(), value: value.into() })
            .collect();
        g.set_levels_readouts(model(readouts));
        g.set_levels_marked(model(key.7.clone()));
        g.set_levels_channel(gestures.levels_channel as i32);
        g.set_levels_modified(modified);
        drawn.levels = Some(key);
    }

    // The wheel, the band tabs and the selected band's three sliders.
    let band = gestures.selected.band;
    let key = (view.spectrogram, sizes[Plot::Wheel as usize], gestures.active, gestures.hovered, band, gestures.band_shown);
    if drawn.wheel.as_ref() != Some(&key) {
        let wheel = spectrogram::Wheel { side: key.1.w };
        let d = spectrogram::drawing(wheel, &view.spectrogram, gestures.active, gestures.hovered);
        g.set_wheel_lines(lines(&d));
        g.set_wheel_dots(dots(&d));
        g.set_band_marked(model(spectrogram::marked(&view.spectrogram)));
        g.set_band(band.index() as i32);
        g.set_spectrogram_modified(view.spectrogram != Spectrogram::default());
        let mut readouts: Vec<SharedString> = Vec::new();
        for field in 0..super::BAND_FIELDS {
            let v = super::band_value(&view.spectrogram, band, field);
            let range = super::band_range(field, band);
            let (at, held) = match gestures.band_shown {
                Some((f, at)) if f == field => (at, true),
                _ => (crate::track::position(v, &(range.0..=range.1), None), false),
            };
            knobs.bands[field].aim(at, jump || scrolled || held, now);
            readouts.push(super::band_readout(field, v).into());
        }
        g.set_band_readouts(model(readouts));
        let (low, high) = band.hue_range();
        g.set_hue_low(hue_end(band, low));
        g.set_hue_high(hue_end(band, high));
        drawn.wheel = Some(key);
        knobs.bands_flying = knobs.bands.iter().any(|k| k.moving(now));
        g.set_band_fractions(model(knobs.bands.iter().map(|k| k.at(now)).collect()));
    } else {
        let flying = knobs.bands.iter().any(|k| k.moving(now));
        let landing = std::mem::replace(&mut knobs.bands_flying, flying);
        if flying || landing {
            g.set_band_fractions(model(knobs.bands.iter().map(|k| k.at(now)).collect()));
        }
    }
    let moving = knobs.levels.iter().chain(&knobs.bands).any(|k| k.moving(now));
    drop(knobs);
    if !moving {
        ctl.editors.springing.stop();
    } else if !ctl.editors.springing.running() {
        let me = Rc::downgrade(ctl);
        ctl.editors.springing.start(TimerMode::Repeated, FRAME, move || {
            if let Some(me) = me.upgrade() {
                draw(&me, false);
            }
        });
    }

    let key = (view.saturation.clone(), sizes[Plot::Saturation as usize], gestures.saturation_dragged);
    if drawn.saturation.as_ref() != Some(&key) {
        let d = saturation::drawing(&key.0, key.2, key.1);
        g.set_saturation_lines(lines(&d));
        g.set_saturation_dots(dots(&d));
        g.set_saturation_modified(!view.saturation.is_neutral());
        drawn.saturation = Some(key);
    }

    let key = (view.print, monochrome, sizes[Plot::Print as usize]);
    if drawn.print.as_ref() != Some(&key) {
        g.set_print_lines(lines(&print::drawing(&key.0, monochrome, key.2)));
        drawn.print = Some(key);
    }
}

/// The gesture state a drawing reads, copied out so no borrow outlives the property writes.
struct Snapshot {
    sizes: [Size; 6],
    curve_channel: usize,
    levels_channel: usize,
    selected: spectrogram::Grip,
    active: Option<spectrogram::Grip>,
    hovered: Option<spectrogram::Grip>,
    held_grip: Option<levels::Grip>,
    saturation_dragged: Option<usize>,
    band_shown: Option<(usize, f32)>,
}

impl Snapshot {
    fn of(g: &Gestures) -> Snapshot {
        Snapshot {
            sizes: g.sizes,
            curve_channel: g.curve_channel.min(curve::CHANNELS.len() - 1),
            levels_channel: g.levels_channel.min(levels::CHANNELS.len() - 1),
            selected: g.selected,
            active: g.active(),
            hovered: g.hovered,
            held_grip: g.held_grip(),
            saturation_dragged: g.saturation_dragged(),
            band_shown: g.band_shown,
        }
    }
}

/// Runs a gesture on the store, keeping the thumbnails' decodes back while a press holds.
fn run(ctl: &Rc<Ctl>, f: impl FnOnce(&mut Gestures, &mut dyn crate::store::Store) -> Then) -> Then {
    let mut s = ctl.state.borrow_mut();
    let mut g = ctl.editors.gestures.borrow_mut();
    let before = g.holding();
    let then = f(&mut g, s.store.as_mut());
    match (before, g.holding()) {
        (false, true) => {
            s.hands.fetch_add(1, Ordering::Relaxed);
        }
        (true, false) => {
            let _ = s.hands.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1));
        }
        _ => {}
    }
    let wait = s.store.settle_if_due();
    drop((g, s));
    if let Some(wait) = wait {
        settle_after(ctl, wait);
    }
    then
}

/// Restarts the settle clock, as each change restarts Swift's recording task.
fn settle_after(ctl: &Rc<Ctl>, wait: std::time::Duration) {
    let me = Rc::downgrade(ctl);
    ctl.editors.settling.start(TimerMode::SingleShot, wait, move || {
        let Some(me) = me.upgrade() else { return };
        let wait = me.state.borrow_mut().store.settle_if_due();
        match wait {
            Some(wait) => settle_after(&me, wait),
            None => me.publish(false),
        }
    });
}

fn after(ctl: &Rc<Ctl>, then: Then) {
    match then {
        Then::Nothing => {}
        Then::Draw => draw(ctl, false),
        Then::Publish => ctl.publish(false),
        Then::Trail(delay) => {
            draw(ctl, false);
            let me = Rc::downgrade(ctl);
            ctl.editors.trailing.start(TimerMode::SingleShot, delay, move || {
                let Some(me) = me.upgrade() else { return };
                let then = run(&me, |g, store| g.trailed(store, Instant::now()));
                after(&me, then);
            });
        }
        Then::Released => {
            ctl.editors.trailing.stop();
            ctl.publish(false);
        }
    }
}

pub fn wire(ctl: &Rc<Ctl>, window: &MainWindow) {
    let g = window.global::<Global>();

    let p = ctl.clone();
    g.on_resized(move |plot, w, h| {
        let Some(plot) = Plot::from_index(plot) else { return };
        let size = Size::new(f64::from(w), f64::from(h));
        {
            let mut gestures = p.editors.gestures.borrow_mut();
            if gestures.sizes[plot as usize] == size {
                return;
            }
            gestures.sizes[plot as usize] = size;
        }
        draw(&p, false);
    });

    let p = ctl.clone();
    g.on_placed(move |plot, x, y| {
        let Some(plot) = Plot::from_index(plot) else { return };
        p.editors.gestures.borrow_mut().origins[plot as usize] = (f64::from(x), f64::from(y));
    });

    let p = ctl.clone();
    g.on_pointer(move |plot, kind, x, y, alt| {
        let Some(plot) = Plot::from_index(plot) else { return };
        let at = (f64::from(x), f64::from(y));
        let now = Instant::now();
        let then = run(&p, |g, store| match kind {
            0 => g.press(store, plot, at, now),
            1 => g.drag(store, plot, at, alt, now),
            2 | 3 => g.release(store, plot),
            4 if plot == Plot::Wheel => g.hover(store, Some(at)),
            5 if plot == Plot::Wheel => g.hover(store, None),
            _ => Then::Nothing,
        });
        after(&p, then);
    });

    let p = ctl.clone();
    g.on_scrolled(move |plot, x, dx, dy, alt| {
        let Some(plot) = Plot::from_index(plot) else { return false };
        let mut claimed = false;
        let then = run(&p, |g, store| match g.scroll(store, plot, f64::from(x), f64::from(dx), f64::from(dy), alt) {
            Some(then) => {
                claimed = true;
                then
            }
            None => Then::Nothing,
        });
        after(&p, then);
        claimed
    });

    let p = ctl.clone();
    g.on_key(move |plot, text, alt, _shift| {
        if Plot::from_index(plot) != Some(Plot::Wheel) {
            return false;
        }
        let mut claimed = false;
        let then = run(&p, |g, store| match g.key(store, &text, alt) {
            Some(then) => {
                claimed = true;
                then
            }
            None => Then::Nothing,
        });
        after(&p, then);
        claimed
    });

    let p = ctl.clone();
    g.on_focus_changed(move |plot, on| {
        if Plot::from_index(plot) != Some(Plot::Wheel) {
            return;
        }
        {
            let mut gestures = p.editors.gestures.borrow_mut();
            if gestures.focused == on {
                return;
            }
            gestures.focused = on;
        }
        draw(&p, false);
    });

    let p = ctl.clone();
    g.on_pick(move |which, tab| {
        let then = p.editors.gestures.borrow_mut().pick(which, tab.max(0) as usize);
        after(&p, then);
    });

    let p = ctl.clone();
    g.on_reset(move |block| {
        let block = match block {
            0 => Reset::Curves,
            1 => Reset::Levels,
            2 => Reset::Spectrogram,
            3 => Reset::Saturation,
            _ => return,
        };
        let then = run(&p, |g, store| g.reset(store, block));
        after(&p, then);
    });

    let p = ctl.clone();
    g.on_slider_press(move |field, _fine| {
        let mut double = false;
        let then = run(&p, |g, store| {
            let (d, then) = g.band_press(store, field.max(0) as usize, Instant::now());
            double = d;
            then
        });
        after(&p, then);
        double
    });

    let p = ctl.clone();
    g.on_slider_drag(move |field, translation, span, fine| {
        let then = run(&p, |g, store| g.band_drag(store, field.max(0) as usize, translation, span, fine, Instant::now()));
        after(&p, then);
    });

    let p = ctl.clone();
    g.on_slider_release(move |_field| {
        let then = run(&p, |g, store| g.band_release(store));
        after(&p, then);
    });

    let p = ctl.clone();
    g.on_slider_scroll(move |field, dx, dy, span, fine| {
        let mut claimed = false;
        let then = run(&p, |g, store| {
            claimed = g.band_scroll(store, field.max(0) as usize, dx, dy, span, fine);
            if claimed { Then::Publish } else { Then::Nothing }
        });
        after(&p, then);
        claimed
    });

    let p = ctl.clone();
    g.on_slider_step(move |field, direction| {
        let then = run(&p, |g, store| g.band_step(store, field.max(0) as usize, direction));
        after(&p, then);
    });

    // The plots laid out before these callbacks existed report their size and place again.
    g.set_epoch(g.get_epoch() + 1);
}

/// Where a plot sits in the window, for a check that aims the pointer at it.
pub fn origin(ctl: &Ctl, plot: Plot) -> (f64, f64) {
    ctl.editors.gestures.borrow().origins[plot as usize]
}

/// A plot's laid-out size.
pub fn size(ctl: &Ctl, plot: Plot) -> Size {
    ctl.editors.gestures.borrow().sizes[plot as usize]
}

/// The store writes the editors made, the hand's cadence's check.
pub fn writes(ctl: &Ctl) -> usize {
    ctl.editors.gestures.borrow().writes
}
