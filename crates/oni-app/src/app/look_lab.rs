//! The Look Lab's sliders: each a distance from the measured Frontier, written as the frame's
//! `frontierLab`, one undo step a press as the store's own sliders take it.

use crate::app::Ctl;
use crate::track::{self, DOUBLE_CLICK, Hand, Offer};
use crate::{LookLab, MainWindow};
use oni_core::Command;
use oni_pipeline::look_lab::measure::{encoded, linear_rgb};
use oni_settings::PipelineSettings;
use oni_settings::frontier_lab::{FrontierLab, GREYS, LabValue, SECTORS};
use slint::{Color, ComponentHandle, ModelRc, SharedString, Timer, TimerMode, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::time::Instant;

/// The sliders, numbered as the panel names them.
pub const SLIDERS: usize = 17;

/// The press under way and its pacing, kept on the interface thread the panel lives on.
#[derive(Default)]
struct Gesture {
    held: Option<(usize, f32)>,
    live: Option<FrontierLab>,
    presses: [Option<Instant>; SLIDERS],
    hand: Hand,
    sector: usize,
    grey: usize,
}

thread_local! {
    static GESTURE: RefCell<Gesture> = RefCell::default();
    static TRAILING: Timer = Timer::default();
}

/// Slider `i` with the sector and grey tab chosen.
pub fn value(i: usize, sector: usize, grey: usize) -> LabValue {
    match i {
        0 => LabValue::Hue(sector),
        1 => LabValue::Twist(sector),
        2 => LabValue::Chroma(sector),
        3 => LabValue::SaturationByL,
        4..=9 => LabValue::Matrix(i - 4),
        10 | 11 => LabValue::PaperGain(i - 10),
        12 | 13 => LabValue::PaperOffset(i - 12),
        14 => LabValue::GreyA(grey),
        15 => LabValue::GreyB(grey),
        _ => LabValue::Character,
    }
}

/// The distance as the panel reads it: degrees, a share of chroma, density, or the character whole.
pub fn readout(v: LabValue, x: f32) -> String {
    match v {
        LabValue::Hue(_) | LabValue::Twist(_) => format!("{x:+.1}°"),
        LabValue::Chroma(_) => format!("{:+.0} %", x * 100.0),
        LabValue::GreyA(_) | LabValue::GreyB(_) => format!("{:+.1}", x * 1000.0),
        LabValue::Character => format!("{:.0} %", (1.0 + x) * 100.0),
        _ => format!("{x:+.3}"),
    }
}

fn fraction(v: LabValue, x: f32) -> f32 {
    let (low, high) = v.range();
    track::position(x, &(low..=high), None)
}

/// Which of colours, dyes, paper, greys and character a slider belongs to.
fn section(i: usize) -> usize {
    match i {
        0..=3 => 0,
        4..=9 => 1,
        10..=13 => 2,
        14 | 15 => 3,
        _ => 4,
    }
}

/// Every value of `section` back on the machine's.
pub fn rested(lab: &FrontierLab, section: usize) -> FrontierLab {
    let mut out = *lab;
    for i in 0..SLIDERS {
        if self::section(i) == section {
            let all = if matches!(i, 0..=2) { SECTORS } else if matches!(i, 14 | 15) { GREYS } else { 1 };
            (0..all).for_each(|k| out.set(value(i, k, k), 0.0));
        }
    }
    out
}

/// A tab's colour: its sector's centre hue at a mid lightness and chroma, in display code.
fn swatch(sector: usize) -> Color {
    let h = (sector as f64 * 30.0).to_radians();
    let rgb = linear_rgb([0.7, 0.12 * h.cos(), 0.12 * h.sin()]).map(|v| (encoded(v.clamp(0.0, 1.0)) * 255.0).round() as u8);
    Color::from_rgb_u8(rgb[0], rgb[1], rgb[2])
}

impl Ctl {
    /// The sliders, tabs and marks of the frame's edits.
    pub(in crate::app) fn publish_look_lab(&self, stored: &FrontierLab) {
        let Some(w) = self.window() else { return };
        let g = w.global::<LookLab>();
        let (live, sector, grey) = GESTURE.with_borrow(|s| (s.live, s.sector, s.grey));
        let lab = live.unwrap_or(*stored);
        let values: Vec<LabValue> = (0..SLIDERS).map(|i| value(i, sector, grey)).collect();
        let fractions: Vec<f32> = values.iter().map(|v| fraction(*v, lab.get(*v))).collect();
        let readouts: Vec<SharedString> = values.iter().map(|v| readout(*v, lab.get(*v)).into()).collect();
        let marked = |n: usize, at: fn(usize) -> [LabValue; 3]| -> Vec<bool> { (0..n).map(|k| at(k).iter().any(|v| lab.get(*v) != 0.0)).collect() };
        let modified: Vec<bool> = (0..5).map(|s| rested(&lab, s) != lab).collect();
        g.set_fractions(ModelRc::new(VecModel::from(fractions)));
        g.set_readouts(ModelRc::new(VecModel::from(readouts)));
        g.set_sector_marked(ModelRc::new(VecModel::from(marked(SECTORS, |k| [LabValue::Hue(k), LabValue::Twist(k), LabValue::Chroma(k)]))));
        g.set_tone_marked(ModelRc::new(VecModel::from(marked(GREYS, |k| [LabValue::GreyA(k), LabValue::GreyB(k), LabValue::GreyA(k)]))));
        g.set_modified(ModelRc::new(VecModel::from(modified)));
        g.set_sector(sector as i32);
        g.set_tone(grey as i32);
    }

    /// One edit of the frame's Lab, written as the whole settings like the editors write theirs.
    fn write_look_lab(self: &Rc<Self>, lab: FrontierLab) {
        let next = PipelineSettings { frontier_lab: lab, ..self.state.borrow().store.settings().clone() };
        self.write(Command::Write(Box::new(next)));
    }

    fn look_lab_once(self: &Rc<Self>, lab: FrontierLab) {
        self.state.borrow_mut().store.defer();
        self.write_look_lab(lab);
        self.schedule_write();
    }

    fn look_lab_press(self: &Rc<Self>, i: usize) -> bool {
        let now = Instant::now();
        let lab = self.state.borrow().store.settings().frontier_lab;
        let (double, v) = GESTURE.with_borrow_mut(|s| {
            let double = s.presses[i].is_some_and(|t| now.duration_since(t) <= DOUBLE_CLICK);
            s.presses[i] = Some(now);
            let v = value(i, s.sector, s.grey);
            if !double {
                s.held = Some((i, fraction(v, lab.get(v))));
                s.live = Some(lab);
            }
            (double, v)
        });
        if double {
            let mut rest = lab;
            rest.set(v, 0.0);
            self.look_lab_once(rest);
            return true;
        }
        let mut s = self.state.borrow_mut();
        s.store.hold();
        s.hands.fetch_add(1, Ordering::Relaxed);
        false
    }

    fn look_lab_drag(self: &Rc<Self>, i: usize, translation: f32, span: f32, fine: bool) {
        let now = Instant::now();
        let offered = GESTURE.with_borrow_mut(|s| {
            let (Some((held, start)), Some(mut lab)) = (s.held, s.live) else { return None };
            if held != i {
                return None;
            }
            let v = value(i, s.sector, s.grey);
            let (low, high) = v.range();
            lab.set(v, track::value(track::clamped(track::asked(start, translation, span, fine)), &(low..=high), None));
            s.live = Some(lab);
            Some((lab, s.hand.offer(now)))
        });
        let Some((lab, offer)) = offered else { return };
        match offer {
            Offer::Now => self.write_look_lab(lab),
            Offer::After(delay) => {
                let q = Rc::downgrade(self);
                TRAILING.with(|t| {
                    t.start(TimerMode::SingleShot, delay, move || {
                        let Some(q) = q.upgrade() else { return };
                        let live = GESTURE.with_borrow_mut(|s| {
                            s.hand.trailed(Instant::now());
                            s.live
                        });
                        if let Some(live) = live {
                            q.write_look_lab(live);
                        }
                    })
                });
            }
            Offer::Wait => self.publish_look_lab(&lab),
        }
    }

    fn look_lab_release(self: &Rc<Self>) {
        let live = GESTURE.with_borrow_mut(|s| {
            s.hand.released();
            s.held.take().and(s.live.take())
        });
        TRAILING.with(Timer::stop);
        let Some(live) = live else { return };
        let _ = self.state.borrow().hands.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1));
        self.write_look_lab(live);
        self.state.borrow_mut().store.release();
        self.publish(false);
        self.schedule_redraw();
        self.schedule_write();
    }

    /// A scroll along the track or an accessibility step: one write, its step closing once it rests.
    fn look_lab_nudge(self: &Rc<Self>, i: usize, by: impl Fn(LabValue, f32) -> f32) {
        let mut lab = self.state.borrow().store.settings().frontier_lab;
        let v = GESTURE.with_borrow(|s| value(i, s.sector, s.grey));
        lab.set(v, by(v, lab.get(v)));
        self.look_lab_once(lab);
    }

    pub(in crate::app) fn wire_look_lab(me: &Rc<Ctl>, window: &MainWindow) {
        let g = window.global::<LookLab>();
        g.set_swatches(ModelRc::new(VecModel::from((0..SECTORS).map(swatch).collect::<Vec<_>>())));
        let slider = |i: i32| usize::try_from(i).ok().filter(|i| *i < SLIDERS);
        let p = me.clone();
        g.on_press(move |i, _fine| slider(i).is_some_and(|i| p.look_lab_press(i)));
        let p = me.clone();
        g.on_drag(move |i, translation, span, fine| {
            if let Some(i) = slider(i) {
                p.look_lab_drag(i, translation, span, fine);
            }
        });
        let p = me.clone();
        g.on_release(move |_| p.look_lab_release());
        let p = me.clone();
        g.on_scroll(move |i, dx, dy, span, fine| {
            let Some(i) = slider(i).filter(|_| dx != 0.0 && dx.abs() > dy.abs()) else { return false };
            p.scroll_kept_alive();
            p.look_lab_nudge(i, |v, x| {
                let (low, high) = v.range();
                let from = fraction(v, x);
                track::value(track::clamped(track::asked(from, Ctl::scroll_travel(dx), span, fine)), &(low..=high), None)
            });
            true
        });
        let p = me.clone();
        g.on_step(move |i, direction| {
            if let Some(i) = slider(i) {
                p.look_lab_nudge(i, |v, x| x + direction as f32 * (v.range().1 - v.range().0) / 100.0);
            }
        });
        let p = me.clone();
        g.on_reset(move |section| {
            let lab = p.state.borrow().store.settings().frontier_lab;
            p.look_lab_once(rested(&lab, section.max(0) as usize));
        });
        let p = me.clone();
        g.on_pick_sector(move |k| {
            GESTURE.with_borrow_mut(|s| s.sector = (k.max(0) as usize).min(SECTORS - 1));
            p.publish(false);
        });
        let p = me.clone();
        g.on_pick_tone(move |k| {
            GESTURE.with_borrow_mut(|s| s.grey = (k.max(0) as usize).min(GREYS - 1));
            p.publish(false);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_slider_reaches_its_value_and_a_section_rests_alone() {
        assert_eq!(value(0, 8, 2), LabValue::Hue(8));
        assert_eq!(value(15, 8, 2), LabValue::GreyB(2));
        assert_eq!(value(16, 0, 0), LabValue::Character);
        let mut lab = FrontierLab::default();
        lab.set(LabValue::Hue(8), -8.0);
        lab.set(LabValue::Matrix(2), 0.05);
        let rest = rested(&lab, 0);
        assert_eq!((rest.hue[8], rest.matrix[2]), (0.0, 0.05), "the colours rest, the dyes stay");
        assert_eq!(fraction(LabValue::Hue(0), 0.0), 0.5, "zero, the machine, sits mid-track");
        assert_eq!(readout(LabValue::Character, -0.25), "75 %");
        // Twin: resting the dyes leaves the colours' edit.
        assert_eq!(rested(&lab, 1).hue[8], -8.0);
    }
}
