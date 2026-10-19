//! Every plot drawn from a snapshot of the gestures and the store, a frame at a time while a spring moves.

use super::*;

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

pub(super) fn draw(ctl: &Rc<Ctl>, force: bool) {
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
    let modified = crate::editors::rested_levels(&view) != view.levels;
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
        for field in 0..crate::editors::BAND_FIELDS {
            let v = crate::editors::band_value(&view.spectrogram, band, field);
            let range = crate::editors::band_range(field, band);
            let (at, held) = match gestures.band_shown {
                Some((f, at)) if f == field => (at, true),
                _ => (crate::track::position(v, &(range.0..=range.1), None), false),
            };
            knobs.bands[field].aim(at, jump || scrolled || held, now);
            readouts.push(crate::editors::band_readout(field, v).into());
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
    pub(super) fn of(g: &Gestures) -> Snapshot {
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
