//! The Look Lab's paper curve on its plot: the machine's, a print without its look, and the frame's,
//! whose five handles follow the hand up and down between their neighbours, one undo step a press.

use crate::app::Ctl;
use crate::track::DOUBLE_CLICK;
use crate::{Ink, LookLab, MainWindow, PlotDot, PlotLine};
use oni_pipeline::look_lab::paper::{self, Curve, NODES};
use oni_pipeline::look_lab::edits::{self, HANDLES};
use oni_pipeline::look_lab::FITTED;
use oni_settings::frontier_lab::{FrontierLab, LabValue, PAPER};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::time::Instant;

/// Points a trace is drawn through, and a node's disc.
const STEPS: usize = 120;
const DOT: f32 = 9.0;

#[derive(Default)]
struct Paper {
    size: (f32, f32),
    held: Option<usize>,
    pressed: Option<(usize, Instant)>,
}

thread_local! {
    static GRAPH: RefCell<Paper> = RefCell::default();
}

fn x_of(d: f64, w: f32) -> f32 {
    ((d - paper::FIRST) / (paper::LAST - paper::FIRST)) as f32 * w
}

fn y_of(code: f64, h: f32) -> f32 {
    (1.0 - code.clamp(0.0, 1.0)) as f32 * h
}

/// The curve as path commands across the plot.
pub fn trace(nodes: &[f64; NODES], (w, h): (f32, f32)) -> String {
    let curve = Curve::new(nodes);
    (0..=STEPS)
        .map(|k| {
            let d = paper::FIRST + (paper::LAST - paper::FIRST) * k as f64 / STEPS as f64;
            format!("{} {:.2} {:.2}", if k == 0 { "M" } else { "L" }, x_of(d, w), y_of(curve.at(d), h))
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The frame's curve at each handle's density.
fn at_handles(lab: &FrontierLab) -> [f64; PAPER] {
    let curve = Curve::new(&edits::edited(lab).paper);
    HANDLES.map(|d| curve.at(d))
}

/// The handle nearest a point across the plot.
pub fn nearest(x: f32, w: f32) -> usize {
    let at = |d: f64| (x_of(d, w) - x).abs();
    (0..PAPER).min_by(|a, b| at(HANDLES[*a]).total_cmp(&at(HANDLES[*b]))).unwrap_or(0)
}

/// The edit that puts handle `i` at `code`, held between its neighbours so the curve keeps climbing.
pub fn edit_at(lab: &FrontierLab, i: usize, code: f64) -> f32 {
    let y = at_handles(lab);
    let low = if i == 0 { 0.0 } else { y[i - 1] };
    let high = if i == PAPER - 1 { 1.0 } else { y[i + 1] };
    (f64::from(lab.paper[i]) + code.clamp(low, high) - y[i]) as f32
}

impl Ctl {
    pub(in crate::app) fn publish_look_lab_paper(&self, lab: &FrontierLab) {
        let Some(w) = self.window() else { return };
        let (size, held) = GRAPH.with_borrow(|p| (p.size, p.held));
        if size.0 <= 0.0 {
            return;
        }
        let edited = edits::edited(lab).paper;
        let line = |nodes: &[f64; NODES], ink: Ink, width: f32| PlotLine { commands: trace(nodes, size).into(), ink, width, opacity: 1.0 };
        let lines = vec![line(&FITTED.neutral.paper, Ink::GreyHalf, 1.0), line(&FITTED.frontier.paper, Ink::GuideFaint, 1.0), line(&edited, Ink::Primary, 2.0)];
        let handles = at_handles(lab);
        let dots: Vec<PlotDot> = (0..PAPER)
            .map(|i| PlotDot {
                x: x_of(HANDLES[i], size.0),
                y: y_of(handles[i], size.1),
                size: DOT,
                fill: if held == Some(i) { Ink::Active } else { Ink::Idle },
                edge: Ink::Canvas,
                edge_width: 1.0,
                halo: 0.0,
            })
            .collect();
        let g = w.global::<LookLab>();
        g.set_paper_lines(ModelRc::new(VecModel::from(lines)));
        g.set_paper_dots(ModelRc::new(VecModel::from(dots)));
    }

    /// 0 press, 1 move, 2 release, 3 cancel, as the plot names them.
    fn paper_pointer(self: &Rc<Self>, kind: i32, x: f32, y: f32) {
        let (size, held) = GRAPH.with_borrow(|p| (p.size, p.held));
        let lab = self.state.borrow().store.settings().frontier_lab;
        match kind {
            0 => {
                let i = nearest(x, size.0);
                let now = Instant::now();
                let double = GRAPH.with_borrow_mut(|p| {
                    let double = p.pressed.is_some_and(|(k, t)| k == i && now.duration_since(t) <= DOUBLE_CLICK);
                    p.pressed = Some((i, now));
                    double
                });
                if double {
                    let mut rest = lab;
                    rest.set(LabValue::Paper(i), 0.0);
                    return self.look_lab_once(rest);
                }
                GRAPH.with_borrow_mut(|p| p.held = Some(i));
                let mut s = self.state.borrow_mut();
                s.store.hold();
                s.hands.fetch_add(1, Ordering::Relaxed);
            }
            1 => {
                let Some(i) = held else { return };
                let mut next = lab;
                next.set(LabValue::Paper(i), edit_at(&lab, i, 1.0 - f64::from(y / size.1.max(1.0))));
                if next != lab {
                    self.write_look_lab(next);
                }
            }
            2 | 3 => {
                if GRAPH.with_borrow_mut(|p| p.held.take()).is_none() {
                    return;
                }
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

    pub(in crate::app) fn wire_look_lab_paper(me: &Rc<Ctl>, window: &MainWindow) {
        let g = window.global::<LookLab>();
        let p = me.clone();
        g.on_paper_resized(move |w, h| {
            GRAPH.with_borrow_mut(|s| s.size = (w, h));
            let lab = p.state.borrow().store.settings().frontier_lab;
            p.publish_look_lab_paper(&lab);
        });
        let p = me.clone();
        g.on_paper_pointer(move |kind, x, y| p.paper_pointer(kind, x, y));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_node_follows_the_hand_between_its_neighbours() {
        assert_eq!((nearest(0.0, 280.0), nearest(280.0, 280.0), nearest(x_of(0.75, 280.0), 280.0)), (0, 4, 2));
        let rest = FrontierLab::default();
        let mut lifted = rest;
        lifted.set(LabValue::Paper(0), edit_at(&rest, 0, 0.05));
        assert!((at_handles(&lifted)[0] - 0.05).abs() < 2e-3, "the blacks' handle lands under the hand: {:?}", at_handles(&lifted));
        assert_eq!(trace(&FITTED.frontier.paper, (280.0, 210.0)).matches('L').count(), STEPS);
        // Twin: the mid-tones asked over the highlights stop level with them.
        let over = edit_at(&rest, 2, 0.99);
        assert!((at_handles(&rest)[2] + f64::from(over) - at_handles(&rest)[3]).abs() < 1e-6);
    }
}
