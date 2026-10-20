//! The Lab's curve, a copy of the Curves block on the Frontier's own `frontierCurves`: a press near a point
//! takes it, elsewhere adds one, a double click removes it, one undo step a press; and Frontier 2's paper,
//! drawn the same way.

use crate::app::Ctl;
use crate::editors::curve::{self, CHANNELS};
use crate::editors::geometry::Size;
use crate::editors::shell::ink::{dots, lines};
use crate::track::DOUBLE_CLICK;
use crate::{LookLab, MainWindow};
use oni_core::Command;
use oni_settings::PipelineSettings;
use oni_settings::curves::CurveSet;
use oni_settings::levels::LevelsChannel;
use slint::{ComponentHandle, ModelRc, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::time::Instant;

/// The paper's layers, one a tab.
const LAYERS: [LevelsChannel; 3] = [LevelsChannel::Red, LevelsChannel::Green, LevelsChannel::Blue];

#[derive(Default)]
struct Hand {
    size: Size,
    paper_size: Size,
    channel: usize,
    paper: usize,
    held: Option<(Option<usize>, bool, (f64, f64))>,
    live: Option<CurveSet>,
    pressed: Option<Instant>,
}

thread_local! {
    static HAND: RefCell<Hand> = RefCell::default();
}

impl Ctl {
    /// The Lab's curve and the paper's, as the Curves block draws them.
    pub(in crate::app) fn publish_look_lab_curves(&self) {
        let Some(w) = self.window() else { return };
        let g = w.global::<LookLab>();
        let stored = self.state.borrow().store.settings().frontier_curves.clone();
        HAND.with_borrow(|h| {
            let curves = h.live.clone().unwrap_or(stored);
            let channel = CHANNELS[h.channel];
            if h.size.w > 0.0 {
                let d = curve::drawing(curves.get(channel), channel, h.size);
                g.set_curve_lines(lines(&d));
                g.set_curve_dots(dots(&d));
            }
            g.set_curve_marked(ModelRc::new(VecModel::from(curve::marked(&curves))));
            g.set_curve_channel(h.channel as i32);
            if h.paper_size.w > 0.0 {
                let layer = LAYERS[h.paper];
                let d = curve::drawing(oni_pipeline::paper_look::CURVES.get(layer), layer, h.paper_size);
                g.set_paper_lines(lines(&d));
                g.set_paper_dots(dots(&d));
            }
            g.set_paper_channel(h.paper as i32);
        });
    }

    fn write_look_lab_curves(self: &Rc<Self>, curves: CurveSet) {
        let next = PipelineSettings { frontier_curves: curves, ..self.state.borrow().store.settings().clone() };
        self.write(Command::Write(Box::new(next)));
    }

    /// 0 press, 1 move, 2 release, 3 cancel.
    fn look_lab_curve_pointer(self: &Rc<Self>, kind: i32, at: (f64, f64)) {
        let channel = HAND.with_borrow(|h| CHANNELS[h.channel]);
        match kind {
            0 => {
                let now = Instant::now();
                let mut curves = self.state.borrow().store.settings().frontier_curves.clone();
                let size = HAND.with_borrow(|h| h.size);
                let index = curve::press(curves.get_mut(channel), at, size);
                HAND.with_borrow_mut(|h| {
                    let double = h.pressed.is_some_and(|t| now.duration_since(t) <= DOUBLE_CLICK);
                    h.pressed = Some(now);
                    h.held = Some((index, double, at));
                    h.live = Some(curves.clone());
                });
                {
                    let mut s = self.state.borrow_mut();
                    s.store.hold();
                    s.hands.fetch_add(1, Ordering::Relaxed);
                }
                self.write_look_lab_curves(curves);
            }
            1 => {
                let next = HAND.with_borrow_mut(|h| {
                    let (Some((Some(index), _, _)), Some(mut curves)) = (h.held, h.live.clone()) else { return None };
                    curve::drag(curves.get_mut(channel), index, at, h.size);
                    h.live = Some(curves.clone());
                    Some(curves)
                });
                if let Some(curves) = next {
                    self.write_look_lab_curves(curves);
                }
            }
            2 | 3 => {
                let done = HAND.with_borrow_mut(|h| {
                    let (held, live) = (h.held.take(), h.live.take());
                    let (Some((_, double, pressed_at)), Some(mut curves)) = (held, live) else { return None };
                    if double && let Some(i) = curve::dot_under(curves.get(channel), pressed_at, h.size) {
                        curves.get_mut(channel).remove(i);
                    }
                    Some(curves)
                });
                let Some(curves) = done else { return };
                self.write_look_lab_curves(curves);
                {
                    let mut s = self.state.borrow_mut();
                    let _ = s.hands.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1));
                    s.store.release();
                }
                self.publish(false);
                self.schedule_redraw();
                self.schedule_write();
            }
            _ => {}
        }
    }

    pub(in crate::app) fn wire_look_lab_curves(me: &Rc<Ctl>, window: &MainWindow) {
        let g = window.global::<LookLab>();
        let p = me.clone();
        g.on_curve_resized(move |w, h| {
            HAND.with_borrow_mut(|s| s.size = Size::new(f64::from(w), f64::from(h)));
            p.publish_look_lab_curves();
        });
        let p = me.clone();
        g.on_curve_pick(move |i| {
            HAND.with_borrow_mut(|s| s.channel = (i.max(0) as usize).min(CHANNELS.len() - 1));
            p.publish_look_lab_curves();
        });
        let p = me.clone();
        g.on_curve_pointer(move |kind, x, y| p.look_lab_curve_pointer(kind, (f64::from(x), f64::from(y))));
        let p = me.clone();
        g.on_paper_resized(move |w, h| {
            HAND.with_borrow_mut(|s| s.paper_size = Size::new(f64::from(w), f64::from(h)));
            p.publish_look_lab_curves();
        });
        let p = me.clone();
        g.on_paper_pick(move |i| {
            HAND.with_borrow_mut(|s| s.paper = (i.max(0) as usize).min(LAYERS.len() - 1));
            p.publish_look_lab_curves();
        });
    }
}
